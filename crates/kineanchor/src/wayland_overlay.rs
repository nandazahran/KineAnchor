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
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
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
    output: Option<wl_output::WlOutput>,
    width: u32,
    height: u32,
    configured: bool,
    needs_redraw: bool,
    last_dx: f32,
    last_dy: f32,
    last_r: f32,
    last_alpha: f32,
    last_count: usize,
    last_color: [u8; 4],
    /// Bounded ring of reusable buffers with the size each was allocated for. A buffer is drawn
    /// into only after its `wl_buffer.release`; one still in flight is skipped.
    buffers: Vec<SurfaceBuffer>,
}

/// Maximum shm buffers kept per surface. Two is the minimum that lets a frame be drawn while
/// the previous one is still on screen; three absorbs a compositor that lags a frame.
const MAX_SURFACE_BUFFERS: usize = 3;

struct SurfaceBuffer {
    buffer: Buffer,
    width: u32,
    height: u32,
}

impl SurfaceState {
    /// Renders one frame, or skips it when every buffer is still owned by the compositor.
    ///
    /// A `wl_buffer` is only released after a later commit attaches a different one, so a single
    /// buffer deadlocks. This keeps a small pool and reuses the first released buffer whose size
    /// matches; the pool never grows past [`MAX_SURFACE_BUFFERS`].
    fn render(&mut self, dots: &[Dot], color: [u8; 4], edge_band: f32) -> bool {
        let (width, height) = (self.width, self.height);

        // Drop buffers that no longer match a resized surface.
        self.buffers
            .retain(|b| b.width == width && b.height == height);

        let ready = self
            .buffers
            .iter()
            .position(|b| !b.buffer.slot().has_active_buffers());

        let index = match ready {
            Some(i) => i,
            None if self.buffers.len() < MAX_SURFACE_BUFFERS => {
                let stride = width as i32 * 4;
                match self.pool.create_buffer(
                    width as i32,
                    height as i32,
                    stride,
                    wl_shm::Format::Argb8888,
                ) {
                    Ok((buffer, _)) => {
                        self.buffers.push(SurfaceBuffer {
                            buffer,
                            width,
                            height,
                        });
                        self.buffers.len() - 1
                    }
                    Err(_) => return false,
                }
            }
            None => return false,
        };

        let Some(entry) = self.buffers.get(index) else {
            return false;
        };
        let Some(canvas) = entry.buffer.canvas(&mut self.pool) else {
            return false;
        };
        clear_edge_bands(canvas, width, height, edge_band);
        render_dots_to_argb(canvas, width, height, dots, color);

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, width as i32, height as i32);
        let attached = self.buffers[index].buffer.attach_to(surface).is_ok();
        if attached {
            self.layer.commit();
        }
        attached
    }
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
            output,
            width: 1920,
            height: 1080,
            configured: false,
            needs_redraw: true,
            last_dx: 0.0,
            last_dy: 0.0,
            last_r: 0.0,
            last_alpha: 0.0,
            last_count: 0,
            last_color: [0; 4],
            buffers: Vec::new(),
        });
    }

    pub fn tick_and_draw(&mut self) {
        let now = crate::monotonic_secs();

        for surface in &mut self.surfaces {
            if !surface.configured || surface.width == 0 || surface.height == 0 {
                continue;
            }

            let width = surface.width;
            let height = surface.height;

            let (dots, color, edge_band) = {
                let mut engine = self.engine.lock().unwrap();
                let color = engine.config().color;
                let edge_band = engine.config().edge_band;
                let (dots_ref, _) = engine.tick(now, [width as f32, height as f32]);
                (dots_ref.to_vec(), color, edge_band)
            };

            let (current_dx, current_dy, current_r, current_alpha) = dots
                .first()
                .map_or((0.0, 0.0, 0.0, 0.0), |d| (d.x, d.y, d.r, d.alpha));
            let is_changed = (current_dx - surface.last_dx).abs() > 0.25
                || (current_dy - surface.last_dy).abs() > 0.25
                || (current_r - surface.last_r).abs() > 0.05
                || (current_alpha - surface.last_alpha).abs() > 0.01
                || dots.len() != surface.last_count
                || color != surface.last_color;

            if !is_changed && !surface.needs_redraw {
                continue;
            }

            surface.last_dx = current_dx;
            surface.last_dy = current_dy;
            surface.last_r = current_r;
            surface.last_alpha = current_alpha;
            surface.last_count = dots.len();
            surface.last_color = color;
            surface.needs_redraw = false;

            // Bounded and allocation-free once warm: one reusable buffer per surface, redrawn
            // only after the compositor released it. A still-active buffer skips this frame.
            surface.render(&dots, color, edge_band);
        }
    }
}

/// Blocks until `fd` is readable or `timeout` elapses. `Ok(false)` means the timeout expired.
fn wait_readable(fd: std::os::fd::BorrowedFd<'_>, timeout: Duration) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;

    let mut poll_fd = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;

    // SAFETY: `poll_fd` points at one initialized pollfd and the count matches.
    let ready = unsafe { libc::poll(&mut poll_fd, 1, millis) };
    if ready < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(err);
    }
    Ok(ready > 0)
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
        // Outputs announced after startup (hotplug) get exactly one surface each.
        if self
            .surfaces
            .iter()
            .any(|s| s.output.as_ref() == Some(&output))
        {
            return;
        }
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
        output: wl_output::WlOutput,
    ) {
        // Remove exactly the gone output; other monitors must keep their overlays. Dropping the
        // LayerSurface and its pool releases the wl_surface, wl_buffer and shm mapping with it.
        self.surfaces.retain(|s| s.output.as_ref() != Some(&output));
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

/// Spawns the layer-shell overlay thread.
///
/// `running` is the public on/off switch. `on_exit` is set when the thread stops for any reason
/// (compositor disconnect, protocol error, or a request to stop), so `main` can tear the process
/// down instead of leaving a live-looking app with no overlay.
pub fn spawn_wayland_overlay(
    engine: Arc<Mutex<Engine>>,
    running: Arc<AtomicBool>,
    on_exit: Arc<AtomicBool>,
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
            // 50 Hz: matches the sensor stream and bounds the wait for compositor events, so a
            // quiet desktop still animates instead of blocking inside blocking_dispatch().
            let target_interval = Duration::from_millis(20);

            loop {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                let start = Instant::now();

                // Flush outgoing requests; a dead connection ends the overlay instead of
                // spinning on a closed socket.
                if conn.flush().is_err() {
                    eprintln!("Wayland overlay: connection lost while flushing; stopping.");
                    running.store(false, Ordering::Relaxed);
                    break;
                }

                // Wait for socket readability without blocking past one frame.
                match conn.prepare_read() {
                    Some(guard) => match wait_readable(guard.connection_fd(), target_interval) {
                        Ok(true) => {
                            if guard.read().is_err() {
                                eprintln!("Wayland overlay: read failed; stopping.");
                                running.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                        // Timeout: cancel the prepared read and keep rendering.
                        Ok(false) => drop(guard),
                        Err(e) => {
                            drop(guard);
                            eprintln!("Wayland overlay: poll failed ({e}); stopping.");
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    },
                    None => std::thread::sleep(target_interval),
                }

                // Dispatch whatever arrived (including buffer releases) before drawing.
                if event_queue.dispatch_pending(&mut app).is_err() {
                    eprintln!("Wayland overlay: dispatch failed; stopping.");
                    running.store(false, Ordering::Relaxed);
                    break;
                }

                app.tick_and_draw();

                let elapsed = start.elapsed();
                if elapsed < target_interval {
                    std::thread::sleep(target_interval - elapsed);
                }
            }

            // Published after every exit path: the caller stops the process when this flips.
            on_exit.store(true, Ordering::Relaxed);
        })
        .map_err(|_| "Failed to spawn Wayland overlay thread")?;

    Ok(handle)
}
