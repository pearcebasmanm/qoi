pub enum Channels {
    Rgb = 3,
    Rgba = 4,
}

impl TryFrom<u8> for Channels {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            3 => Ok(Self::Rgb),
            4 => Ok(Self::Rgba),
            _ => Err(()),
        }
    }
}

pub enum Colorspace {
    Standard = 0,
    Linear = 1,
}

impl TryFrom<u8> for Colorspace {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Standard),
            1 => Ok(Self::Linear),
            _ => Err(()),
        }
    }
}
