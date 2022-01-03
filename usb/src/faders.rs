#[derive(Copy, Clone, Debug)]
pub enum Fader {
    A,
    B,
    C,
    D,
}

impl Fader {
    pub fn id(&self) -> u32 {
        match self {
            Fader::A => 0,
            Fader::B => 1,
            Fader::C => 2,
            Fader::D => 3,
        }
    }
}
