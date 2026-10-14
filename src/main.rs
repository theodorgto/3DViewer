#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod import_job;
mod model;
mod renderer;

fn main() -> eframe::Result {
    let mode = std::env::args_os().nth(1);
    let export = mode.as_deref() == Some(std::ffi::OsStr::new("--export-model"));
    if export || mode.as_deref() == Some(std::ffi::OsStr::new("--import-worker")) {
        let arguments: Vec<_> = std::env::args_os().skip(2).collect();
        let result = std::panic::catch_unwind(|| -> anyhow::Result<()> {
            anyhow::ensure!(arguments.len() == 3, "Invalid import worker arguments");
            let tolerance = arguments[1].to_string_lossy().parse()?;
            let worker = if export {
                import_job::export_worker
            } else {
                import_job::run_worker
            };
            worker(
                std::path::Path::new(&arguments[0]),
                tolerance,
                std::path::Path::new(&arguments[2]),
            )
        });
        match result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => eprintln!("Import failed: {error:#}"),
            Err(_) => eprintln!("Import failed: The model importer panicked"),
        }
        std::process::exit(1);
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--check-model")) {
        let path = std::env::args_os()
            .nth(2)
            .expect("--check-model requires a file path");
        let started = std::time::Instant::now();
        let result = (|| -> anyhow::Result<model::Model> {
            let mut job = import_job::ImportJob::start(std::path::PathBuf::from(path), 0.001)?;
            loop {
                if let Some(model) = job.poll()? {
                    return Ok(model);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        })();
        match result {
            Ok(model) => {
                println!(
                    "Imported {}: {} vertices, {} triangles in {:.2}s",
                    model.name,
                    model.vertices.len(),
                    model.indices.len() / 3,
                    started.elapsed().as_secs_f64()
                );
                for warning in &model.warnings {
                    eprintln!("WARNING: {warning}");
                }
            }
            Err(error) => {
                eprintln!("Import failed: {error:#}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        depth_buffer: 24,
        multisampling: 4,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/3dviewer.png"))
                    .expect("The bundled application icon must be valid PNG"),
            )
            .with_title("Model Viewer")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Model Viewer",
        options,
        Box::new(|context| Ok(Box::new(app::Viewer::new(context)?))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn application_icon_decodes() {
        let icon =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/3dviewer.png")).unwrap();
        assert_eq!((icon.width, icon.height), (512, 512));
        assert_eq!(icon.rgba.len(), 512 * 512 * 4);
        let (pixels, _) = icon.rgba.as_chunks::<4>();
        assert!(pixels.iter().any(|pixel| pixel[3] == 0));
        assert!(pixels.iter().any(|pixel| pixel[3] == 255));
    }
}
