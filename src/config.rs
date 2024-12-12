use std::time::Duration;

pub const GRID_LENGTH: usize = 1;
pub const HOST: &str = "127.0.0.1:7791";
pub const WEB_HOST: &str = "127.0.0.1:3000";
pub const IMAGE_SAVE_INTERVAL: Duration = Duration::from_secs(5);
pub const JPEG_UPDATE_INTERVAL: Duration = Duration::from_millis(17);
pub const WEB_UPDATE_INTERVAL: Duration = Duration::from_millis(50);
pub const AUTH_SERVER_URL: &str = "https://test.auth/";

pub const HELP_TEXT: &[u8] = b"Flurry is a pixelflut implementation, this means you can use commands to get and set pixels in the canvas
SIZE returns the size of the canvas
PX {x} {y} returns the color of the pixel at {x}, {y}
If you include a color in hex format you set a pixel instead
PX {x} {y} {RGB} sets the color of the pixel at {x}, {y} to the rgb value
PX {x} {y} {RGBA} blends the pixel at {x}, {y} with the rgb value weighted by the a
PX {x} {y} {W} sets the color of the pixel at {x}, {y} to the grayscale value
";
