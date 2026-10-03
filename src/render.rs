//! Fullscreen shader renderer built on winit + wgpu.
//!
//! One fullscreen window per active monitor, each drawing a WGSL fragment
//! shader (Shadertoy-style) over a single full-screen triangle. All windows
//! share one GPU device/queue; each has its own surface, pipeline, and
//! uniforms (so per-monitor resolution and format are respected). How it
//! exits depends on [`DismissMode`]: Escape-only for an explicit run, or any
//! input when launched by the idle daemon. Exiting closes every window.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

/// How the saver can be dismissed.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DismissMode {
    /// Explicit `show`: only the Escape key exits. Stray mouse bumps are ignored.
    #[default]
    EscapeOnly,
    /// Idle-triggered: any key, mouse button, or real movement exits — so the
    /// returning user dismisses it however they touch the machine.
    AnyInput,
}

/// Grace period after launch during which input is ignored, so the keypress or
/// mouse jiggle that triggered the saver doesn't dismiss it instantly.
const GRACE: Duration = Duration::from_millis(700);

/// Bundled effect, used when no `--shader` is given.
const DEFAULT_SHADER: &str = include_str!("../shaders/lines.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    time: f32,
    seed: f32,
    resolution: [f32; 2],
}

/// GPU resources shared across every monitor's window.
struct Gpu {
    #[allow(dead_code)] // kept alive for the lifetime of the surfaces
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
}

/// Per-monitor render target.
struct Surf {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    seed: f32,
}

impl Surf {
    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(device, &self.config);
    }

    fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, time: f32) {
        let uniforms = Uniforms {
            time,
            seed: self.seed,
            resolution: [self.config.width as f32, self.config.height as f32],
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(device, &self.config);
                return;
            }
            _ => {
                log::warn!("surface frame unavailable this tick");
                return;
            }
        };

        let view = frame.texture.create_view(&Default::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("saver pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
        queue.present(frame);
    }
}

fn make_window(
    event_loop: &ActiveEventLoop,
    monitor: Option<MonitorHandle>,
) -> anyhow::Result<Arc<Window>> {
    let attrs = Window::default_attributes()
        .with_title("scrnsav")
        .with_fullscreen(Some(Fullscreen::Borderless(monitor)));
    let window = Arc::new(event_loop.create_window(attrs).context("creating window")?);
    window.set_cursor_visible(false);
    Ok(window)
}

/// A stable per-monitor phase offset, so each screen renders a distinct
/// variation of the effect. Keyed on the monitor's name when available (stable
/// across restarts), falling back to its index.
fn monitor_seed(monitor: &MonitorHandle, index: usize) -> f32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match monitor.name() {
        Some(name) => name.hash(&mut hasher),
        None => index.hash(&mut hasher),
    }
    // Map the hash into a few full phase turns.
    (hasher.finish() % 100_000) as f32 / 100_000.0 * std::f32::consts::TAU * 8.0
}

/// Build the per-monitor surface, choosing its own preferred format so each
/// output is configured correctly even if they differ.
fn build_surf(gpu: &Gpu, window: Arc<Window>, surface: wgpu::Surface<'static>, seed: f32) -> Surf {
    let size = window.inner_size();
    let caps = surface.get_capabilities(&gpu.adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        color_space: wgpu::SurfaceColorSpace::default(),
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&gpu.device, &config);

    let uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uniform buffer"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("uniforms"),
        layout: &gpu.bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buf.as_entire_binding(),
        }],
    });

    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("saver pipeline"),
            layout: Some(&gpu.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &gpu.shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &gpu.shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    Surf {
        window,
        surface,
        config,
        pipeline,
        uniform_buf,
        bind_group,
        seed,
    }
}

/// Create the GPU context and one window+surface per active monitor.
fn init(
    event_loop: &ActiveEventLoop,
    shader_src: &str,
) -> anyhow::Result<(Gpu, HashMap<WindowId, Surf>)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    // One fullscreen window per monitor (fall back to a single default window).
    let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
    let mut pairs: Vec<(Arc<Window>, wgpu::Surface<'static>, f32)> = Vec::new();
    if monitors.is_empty() {
        let window = make_window(event_loop, None)?;
        let surface = instance
            .create_surface(window.clone())
            .context("creating wgpu surface")?;
        pairs.push((window, surface, 0.0));
    } else {
        log::info!("spanning {} monitor(s)", monitors.len());
        for (i, monitor) in monitors.into_iter().enumerate() {
            let seed = monitor_seed(&monitor, i);
            let window = make_window(event_loop, Some(monitor))?;
            let surface = instance
                .create_surface(window.clone())
                .context("creating wgpu surface")?;
            pairs.push((window, surface, seed));
        }
    }

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&pairs[0].1),
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .context("no suitable GPU adapter")?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("scrnsav device"),
        required_features: wgpu::Features::empty(),
        // Use what the adapter actually supports; downlevel_defaults caps
        // textures at 2048px, too small for a 2560x1440+ display.
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::default(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .context("requesting GPU device")?;

    // Capture shader-compile and pipeline-creation validation errors (e.g. a
    // broken `--shader` file, or one that parses but exceeds this GPU's limits)
    // instead of letting wgpu's default handler panic. Scope spans every
    // pipeline too, so it must be popped after the surfaces are built below.
    let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("saver shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let gpu = Gpu {
        instance,
        adapter,
        device,
        queue,
        shader,
        bind_group_layout,
        pipeline_layout,
    };

    let mut surfaces = HashMap::new();
    for (window, surface, seed) in pairs {
        let surf = build_surf(&gpu, window, surface, seed);
        surfaces.insert(surf.window.id(), surf);
    }

    if let Some(err) = pollster::block_on(error_scope.pop()) {
        anyhow::bail!("shader failed to compile:\n{err}");
    }

    Ok((gpu, surfaces))
}

#[derive(Default)]
struct App {
    gpu: Option<Gpu>,
    surfaces: HashMap<WindowId, Surf>,
    start: Option<Instant>,
    launched_at: Option<Instant>,
    shader_src: String,
    dismiss: DismissMode,
    /// Set if setup failed inside the event loop, so `run` can report it.
    init_error: Option<anyhow::Error>,
}

impl App {
    fn past_grace(&self) -> bool {
        self.launched_at
            .map(|t| t.elapsed() > GRACE)
            .unwrap_or(false)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        match init(event_loop, &self.shader_src) {
            Ok((gpu, surfaces)) => {
                self.gpu = Some(gpu);
                self.surfaces = surfaces;
                self.start = Some(Instant::now());
                self.launched_at = Some(Instant::now());
            }
            Err(e) => {
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        let past_grace = self.past_grace();
        let dismiss = self.dismiss;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed =>
            {
                let is_escape = ke.physical_key == PhysicalKey::Code(KeyCode::Escape);
                match dismiss {
                    // Escape always exits, grace or not.
                    DismissMode::EscapeOnly if is_escape => event_loop.exit(),
                    // Any key exits once past the grace window.
                    DismissMode::AnyInput if past_grace => event_loop.exit(),
                    _ => {}
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } if dismiss == DismissMode::AnyInput && past_grace => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_ref() {
                    if let Some(surf) = self.surfaces.get_mut(&id) {
                        surf.resize(&gpu.device, size.width, size.height);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(start)) = (self.gpu.as_ref(), self.start) {
                    if let Some(surf) = self.surfaces.get_mut(&id) {
                        surf.render(&gpu.device, &gpu.queue, start.elapsed().as_secs_f32());
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.dismiss == DismissMode::AnyInput {
            if let DeviceEvent::MouseMotion { delta } = event {
                if self.past_grace() && delta.0.abs() + delta.1.abs() > 2.0 {
                    event_loop.exit();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        for surf in self.surfaces.values() {
            surf.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Uniforms;

    #[test]
    fn uniforms_match_wgsl_layout() {
        // Must stay 16 bytes to match `struct Uniforms` in the WGSL shaders.
        assert_eq!(std::mem::size_of::<Uniforms>(), 16);
    }
}

/// Run the saver. `shader` is a path to a WGSL file; when `None`, the bundled
/// default effect is used. `dismiss` selects how it exits.
pub fn run(shader: Option<String>, dismiss: DismissMode) -> anyhow::Result<()> {
    let shader_src = match shader {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading shader file '{path}'"))?,
        None => DEFAULT_SHADER.to_string(),
    };

    let event_loop = EventLoop::new().context("creating event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        shader_src,
        dismiss,
        ..Default::default()
    };
    event_loop.run_app(&mut app).context("running event loop")?;
    if let Some(e) = app.init_error {
        return Err(e);
    }
    Ok(())
}
