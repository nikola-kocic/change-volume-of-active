#[derive(Debug)]
pub enum VolumeOp {
    ToggleMute,
    ChangeVolume(f32),
}
