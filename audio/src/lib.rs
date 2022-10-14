mod audio;
mod player;

#[cfg(target_os = "linux")]
mod pulse;

#[cfg(not(target_os = "linux"))]
mod cpal;
