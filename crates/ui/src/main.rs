use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = rustcraft_ui::build_ui()?;
    ui.run()?;
    Ok(())
}
