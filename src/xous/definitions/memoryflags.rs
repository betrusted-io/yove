/// Flags to be passed to the MapMemory struct.
/// Note that it is an error to have memory be
/// writable and not readable.
#[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord, Hash, Debug)]
pub struct MemoryFlags {
    bits: usize,
}

impl MemoryFlags {
    /// Free this memory
    pub const FREE: Self = Self { bits: 0b0000_0000 };

    /// Immediately allocate this memory.  Otherwise it will
    /// be demand-paged.  This is implicitly set when `phys`
    /// is not 0.
    pub const VALID: Self = Self { bits: 0b0000_0001 };

    /// Allow the CPU to read from this page.
    pub const READ: Self = Self { bits: 0b0000_0010 };

    /// Allow the CPU to write to this page.
    pub const WRITE: Self = Self { bits: 0b0000_0100 };

    /// Allow the CPU to execute from this page.
    pub const EXECUTE: Self = Self { bits: 0b0000_1000 };

    /// Accessible from user mode
    pub const USERMODE: Self = Self { bits: 0b0001_0000 };

    /// Globally-available
    pub const GLOBAL: Self = Self { bits: 0b0010_0000 };

    /// Cache access status
    pub const ACCESSED: Self = Self { bits: 0b0100_0000 };

    /// Page needs flushing
    pub const DIRTY: Self = Self { bits: 0b1000_0000 };

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn from_bits(raw: usize) -> Option<MemoryFlags> {
        if raw > 255 {
            None
        } else {
            Some(MemoryFlags { bits: raw })
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn empty() -> MemoryFlags {
        MemoryFlags { bits: 0 }
    }

    pub fn all() -> MemoryFlags {
        MemoryFlags { bits: 255 }
    }

    pub fn contains(&self, other: MemoryFlags) -> bool {
        (self.bits & other.bits) == other.bits
    }
}

impl core::fmt::Binary for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Binary::fmt(&self.bits, f)
    }
}

impl core::fmt::Octal for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Octal::fmt(&self.bits, f)
    }
}

impl core::fmt::LowerHex for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl core::fmt::UpperHex for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl core::ops::BitOr for MemoryFlags {
    type Output = Self;

    /// Returns the union of the two sets of flags.
    #[inline]
    fn bitor(self, other: MemoryFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl core::ops::BitOrAssign for MemoryFlags {
    /// Adds the set of flags.
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl core::ops::BitXor for MemoryFlags {
    type Output = Self;

    /// Returns the left flags, but with all the right flags toggled.
    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl core::ops::BitXorAssign for MemoryFlags {
    /// Toggles the set of flags.
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl core::ops::BitAnd for MemoryFlags {
    type Output = Self;

    /// Returns the intersection between the two sets of flags.
    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl core::ops::BitAndAssign for MemoryFlags {
    /// Disables all flags disabled in the set.
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl core::ops::Sub for MemoryFlags {
    type Output = Self;

    /// Returns the set difference of the two sets of flags.
    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl core::ops::SubAssign for MemoryFlags {
    /// Disables all flags enabled in the set.
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl core::ops::Not for MemoryFlags {
    type Output = Self;

    /// Returns the complement of this set of flags.
    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits } & MemoryFlags { bits: 255 }
    }
}

impl core::fmt::Display for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        if self.contains(MemoryFlags::FREE) {
            write!(f, "FREE")?;
            first = false;
        }
        if self.contains(MemoryFlags::VALID) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "VALID")?;
            first = false;
        }
        if self.contains(MemoryFlags::READ) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "READ")?;
            first = false;
        }
        if self.contains(MemoryFlags::WRITE) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "WRITE")?;
            first = false;
        }
        if self.contains(MemoryFlags::EXECUTE) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "EXECUTE")?;
            first = false;
        }
        if self.contains(MemoryFlags::USERMODE) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "USERMODE")?;
            first = false;
        }
        if self.contains(MemoryFlags::GLOBAL) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "GLOBAL")?;
            first = false;
        }
        if self.contains(MemoryFlags::ACCESSED) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "ACCESSED")?;
            first = false;
        }
        if self.contains(MemoryFlags::DIRTY) {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "DIRTY")?;
        }
        Ok(())
    }
}
