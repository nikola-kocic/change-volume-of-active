#[derive(Debug)]
pub enum VolumeOp {
    ToggleMute,
    ChangeVolume(f32),
    SetVolume(f32),
}
