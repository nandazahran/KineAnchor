// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
    Connection, QueueHandle,
    globals::{GlobalList, registry_queue_init},
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
};

use kineanchor_core::{Dot, Engine};

struct SurfaceState {
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
    needs_redraw: bool,
    last_dx: f32,
    last_dy: f32,
    last_roll: f32,
}

pub struct WaylandOverlayApp {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,
    surfaces: Vec<SurfaceState>,
    engine: Arc<Mutex<Engine>>,
    start_time: Instant,
}

impl WaylandOverlayApp {
    pub fn is_supported() -> bool {
        let conn = match Connection::connect_to_env() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let (globals, _) = match registry_queue_init::<Self>(&conn) {
            Ok(g) => g,
            Err(_) => return false,
        };
        globals
            .contents()
            .with_list(|list| list.iter().any(|g| g.interface == "zwlr_layer_shell_v1"))
    }

    pub fn new(
        engine: Arc<Mutex<Engine>>,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
    ) -> Result<Self, &'static str> {
        let compositor =
            CompositorState::bind(globals, qh).map_err(|_| "wl_compositor not available")?;
        let layer_shell =
            LayerShell::bind(globals, qh).map_err(|_| "zwlr_layer_shell_v1 not available")?;
        let shm = Shm::bind(globals, qh).map_err(|_| "wl_shm not available")?;
        let output_state = OutputState::new(globals, qh);
        let seat_state = SeatState::new(globals, qh);
        let registry_state = RegistryState::new(globals);

        Ok(Self {
            registry_state,
            output_state,
            seat_state,
            shm,
            compositor,
            layer_shell,
            surfaces: Vec::new(),
            engine,
            start_time: Instant::now(),
        })
    }

    pub fn setup_surfaces(&mut self, qh: &QueueHandle<Self>) {
        let outputs: Vec<wl_output::WlOutput> = self.output_state.outputs().collect();

        if outputs.is_empty() {
            self.create_layer_surface(None, qh);
        } else {
            for output in outputs {
                self.create_layer_surface(Some(output), qh);
            }
        }
    }

    fn create_layer_surface(
        &mut self,
        output: Option<wl_output::WlOutput>,
        qh: &QueueHandle<Self>,
    ) {
        let surface = self.compositor.create_surface(qh);

        // Make surface click-through: empty input region
        let empty_region = smithay_client_toolkit::compositor::Region::new(&self.compositor).ok();
        if let Some(ref region) = empty_region {
            surface.set_input_region(Some(region.wl_region()));
        }

        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("kineanchor_overlay"),
            output.as_ref(),
        );

        // Fill the entire output
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1); // Don't move other windows
        layer.commit();
        drop(empty_region);

        let pool =
            SlotPool::new(1920 * 1080 * 4, &self.shm).expect("Failed to create Shm SlotPool");

        self.surfaces.push(SurfaceState {
            layer,
            pool,
            width: 1920,
            height: 1080,
            configured: false,
            needs_redraw: true,
            last_dx: 0.0,
            last_dy: 0.0,
            last_roll: 0.0,
        });
    }

    pub fn tick_and_draw(&mut self) {
        let now = self.start_time.elapsed().as_secs_f64();

        for surface in &mut self.surfaces {
            if !surface.configured || surface.width == 0 || surface.height == 0 {
                continue;
            }

            let width = surface.width;
            let height = surface.height;
            let stride = width as i32 * 4;

            let (dots, color, edge_band) = {
                let mut engine = self.engine.lock().unwrap();
                let color = engine.config().color;
                let edge_band = engine.config().edge_band;
                let (dots_ref, _) = engine.tick(now, [width as f32, height as f32]);
                (dots_ref.to_vec(), color, edge_band)
            };

            // Calculate motion delta to skip unnecessary redraws when dots are static
            let current_dx = dots.first().map_or(0.0, |d| d.x);
            let current_dy = dots.first().map_or(0.0, |d| d.y);
            let current_roll = dots.first().map_or(0.0, |_| 0.0);
            let is_moving = (current_dx - surface.last_dx).abs() > 0.25
                || (current_dy - surface.last_dy).abs() > 0.25
                || (current_roll - surface.last_roll).abs() > 0.005;

            if !is_moving && !surface.needs_redraw {
                continue;
            }

            surface.last_dx = current_dx;
            surface.last_dy = current_dy;
            surface.last_roll = current_roll;
            surface.needs_redraw = false;

            let (buffer, canvas) = match surface.pool.create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            ) {
                Ok(b) => b,
                Err(_) => continue,
            };

            // Clear peripheral edge bands only (leaving the 70% center untouched)
            clear_edge_bands(canvas, width, height, edge_band);

            // Render anti-aliased filled circles into ARGB buffer
            render_dots_to_argb(canvas, width, height, &dots, color);

            // Present buffer
            surface
                .layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            let _ = buffer.attach_to(surface.layer.wl_surface());
            surface.layer.commit();
        }
    }
}

/// Clears only the peripheral margin areas where dots live, skipping the center screen.
fn clear_edge_bands(canvas: &mut [u8], width: u32, height: u32, edge_band: f32) {
    let band_w = ((width as f32 * edge_band * 1.6).ceil() as u32).min(width / 2);
    let band_h = ((height as f32 * edge_band * 1.6).ceil() as u32).min(height / 2);

    let row_bytes = width as usize * 4;
    let band_w_bytes = band_w as usize * 4;

    for y in 0..height {
        let row_start = y as usize * row_bytes;
        let row_end = row_start + row_bytes;

        if y < band_h || y >= height - band_h {
            // Full row in top or bottom peripheral margin
            canvas[row_start..row_end].fill(0);
        } else {
            // Left band
            canvas[row_start..row_start + band_w_bytes].fill(0);
            // Right band
            canvas[row_end - band_w_bytes..row_end].fill(0);
        }
    }
}

/// Anti-aliased circle rasterizer writing directly to 32-bit ARGB pre-multiplied buffer
fn render_dots_to_argb(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    dots: &[Dot],
    base_color: [u8; 4],
) {
    let [cr, cg, cb, _] = base_color;

    for dot in dots {
        let r = dot.r;
        let min_x = ((dot.x - r - 1.0).floor() as i32).max(0) as u32;
        let max_x = ((dot.x + r + 1.0).ceil() as i32).clamp(0, width as i32 - 1) as u32;
        let min_y = ((dot.y - r - 1.0).floor() as i32).max(0) as u32;
        let max_y = ((dot.y + r + 1.0).ceil() as i32).clamp(0, height as i32 - 1) as u32;

        let total_alpha = dot.alpha.clamp(0.0, 1.0);

        for py in min_y..=max_y {
            let dy = (py as f32 + 0.5) - dot.y;
            let dy2 = dy * dy;

            for px in min_x..=max_x {
                let dx = (px as f32 + 0.5) - dot.x;
                let dist = (dx * dx + dy2).sqrt();

                // Anti-aliasing edge factor
                let coverage = (r + 0.5 - dist).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }

                let alpha = (total_alpha * coverage).clamp(0.0, 1.0);
                let a_val = (alpha * 255.0) as u8;
                let r_val = (cr as f32 * alpha) as u8;
                let g_val = (cg as f32 * alpha) as u8;
                let b_val = (cb as f32 * alpha) as u8;

                let offset = (py as usize * width as usize + px as usize) * 4;
                if offset + 3 < canvas.len() {
                    // Wayland ARGB8888 in little-endian is [B, G, R, A]
                    canvas[offset] = b_val;
                    canvas[offset + 1] = g_val;
                    canvas[offset + 2] = r_val;
                    canvas[offset + 3] = a_val;
                }
            }
        }
    }
}

impl smithay_client_toolkit::compositor::CompositorHandler for WaylandOverlayApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_scale: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandOverlayApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.create_layer_surface(Some(output), qh);
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for WaylandOverlayApp {
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        for s in &mut self.surfaces {
            if &s.layer == layer {
                s.width = configure.new_size.0.max(1);
                s.height = configure.new_size.1.max(1);
                s.configured = true;
                s.needs_redraw = true;
                break;
            }
        }
    }

    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        self.surfaces.retain(|s| &s.layer != layer);
    }
}

impl SeatHandler for WaylandOverlayApp {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _: Capability,
    ) {
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _: Capability,
    ) {
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl ShmHandler for WaylandOverlayApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(WaylandOverlayApp);

impl ProvidesRegistryState for WaylandOverlayApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(WaylandOverlayApp);

pub fn spawn_wayland_overlay(
    engine: Arc<Mutex<Engine>>,
    running: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, &'static str> {
    let conn = Connection::connect_to_env().map_err(|_| "Failed to connect to Wayland display")?;
    let (globals, mut event_queue) = registry_queue_init::<WaylandOverlayApp>(&conn)
        .map_err(|_| "Failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let mut app = WaylandOverlayApp::new(engine, &globals, &qh)?;
    app.setup_surfaces(&qh);

    // Initial dispatch to configure layer surfaces
    let _ = event_queue.roundtrip(&mut app);

    let handle = std::thread::Builder::new()
        .name("wayland-overlay".into())
        .spawn(move || {
            let target_interval = Duration::from_millis(20); // 50 Hz frame rate (matches sensor stream)

            while running.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Dispatch incoming Wayland protocol events
                let _ = event_queue.dispatch_pending(&mut app);

                // Tick physics and render frame
                app.tick_and_draw();
                let _ = conn.flush();

                let elapsed = start.elapsed();
                if elapsed < target_interval {
                    std::thread::sleep(target_interval - elapsed);
                }
            }
        })
        .map_err(|_| "Failed to spawn Wayland overlay thread")?;

    Ok(handle)
}
