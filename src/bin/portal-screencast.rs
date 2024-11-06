use portal_screencast::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen_cast = ScreenCast::new()?;
    // Set which source types to allow, and enable multiple items to be shared.
    screen_cast.set_source_types(SourceType::MONITOR | SourceType::WINDOW);
    screen_cast.enable_multiple();
    // If you have a window handle you can tie the dialog to it
    let screen_cast = screen_cast.start(None)?;

    println!(
        "pipewire node: {:?}",
        screen_cast
            .streams()
            .map(|s| s.pipewire_node())
            .collect::<Vec<_>>()
    );

    std::thread::sleep(std::time::Duration::from_secs(60 * 60 * 12));
    Ok(())
}
