use super::*;
use crate::seesaw_io::SeesawCommand;
use octessera_hal::OledSsd1351;

pub(crate) fn run(
    runtime_config: runtime_thread::RuntimeThreadConfig,
    seesaw_tx: mpsc::Sender<SeesawCommand>,
    hdmi: render::hdmi::HdmiFramebuffer,
) {
    let mut prepared = match runtime_thread::prepare(runtime_config) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("pi runtime preparation failed: {error}");
            return;
        }
    };
    let handoff = match boot_oled_handoff::native_attach() {
        Ok(handoff) => handoff,
        Err(error) => {
            eprintln!("pi OLED boot handoff attach failed: {error}");
            return;
        }
    };
    let oled = match OledSsd1351::adopt_existing() {
        Ok(oled) => oled,
        Err(error) => {
            handoff.mark_failed();
            eprintln!("pi OLED adoption failed: {error}");
            return;
        }
    };
    let render_worker = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx,
        oled_handoff: Some(handoff),
        hdmi,
    });
    let revision = match prepared.publish_acknowledged_snapshot(&render_worker) {
        Ok(revision) => revision,
        Err(error) => {
            let _ = render_worker.mark_oled_failed();
            let _ = render_worker.abort();
            eprintln!("pi initial OLED render failed: {error}");
            return;
        }
    };
    if let Err(error) = render_worker.mark_first_menu_rendered() {
        let _ = render_worker.mark_oled_failed();
        let _ = render_worker.abort();
        eprintln!("pi OLED handoff status failed: {error}");
        return;
    }
    if let Err(error) = prepared.mark_candidate_ready() {
        let _ = render_worker.mark_oled_failed();
        let _ = render_worker.abort();
        eprintln!("pi candidate readiness publication failed: {error}");
        return;
    }
    let runtime = prepared.spawn_after_initial(render_worker, revision);
    if runtime.join().is_err() {
        eprintln!("pi runtime thread panicked");
    }
}
