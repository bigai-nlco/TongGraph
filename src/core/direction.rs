#[derive(Clone, Copy)]
pub(super) enum Direction {
    Out,
    In,
    Both,
}

impl Direction {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "out" | "outgoing" => Ok(Self::Out),
            "in" | "incoming" => Ok(Self::In),
            "both" | "all" => Ok(Self::Both),
            other => Err(format!(
                "direction must be 'out', 'in', or 'both', got {other:?}"
            )),
        }
    }

    pub(super) fn includes_out(self) -> bool {
        matches!(self, Self::Out | Self::Both)
    }

    pub(super) fn includes_in(self) -> bool {
        matches!(self, Self::In | Self::Both)
    }
}
