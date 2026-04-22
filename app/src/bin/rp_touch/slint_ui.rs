slint::include_modules!();

pub fn create_app_ui() -> Result<AppUi, slint::PlatformError> {
    let app = AppUi::new()?;
    app.show()?;
    Ok(app)
}
