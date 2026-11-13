//! Coordinate access for `AtomDatum`.
//!
//! readcon-core 0.21 stores `x`, `y`, and `z` as fields. 0.23 stores one
//! pointer and exposes `x()` / `set_x()`. The campaign build sets
//! `READCON_DB_CORE_0_23=1`; the published pin does not.

use readcon_core::types::AtomDatum;

#[inline]
pub(crate) fn atom_x(atom: &AtomDatum) -> f64 {
    #[cfg(readcon_core_0_23)]
    {
        atom.x()
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.x
    }
}

#[inline]
pub(crate) fn atom_y(atom: &AtomDatum) -> f64 {
    #[cfg(readcon_core_0_23)]
    {
        atom.y()
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.y
    }
}

#[inline]
pub(crate) fn atom_z(atom: &AtomDatum) -> f64 {
    #[cfg(readcon_core_0_23)]
    {
        atom.z()
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.z
    }
}

#[inline]
pub(crate) fn atom_xyz(atom: &AtomDatum) -> [f64; 3] {
    [atom_x(atom), atom_y(atom), atom_z(atom)]
}

#[inline]
pub(crate) fn set_atom_x(atom: &mut AtomDatum, value: f64) {
    #[cfg(readcon_core_0_23)]
    {
        atom.set_x(value);
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.x = value;
    }
}

#[inline]
pub(crate) fn set_atom_y(atom: &mut AtomDatum, value: f64) {
    #[cfg(readcon_core_0_23)]
    {
        atom.set_y(value);
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.y = value;
    }
}

#[inline]
pub(crate) fn set_atom_z(atom: &mut AtomDatum, value: f64) {
    #[cfg(readcon_core_0_23)]
    {
        atom.set_z(value);
    }
    #[cfg(not(readcon_core_0_23))]
    {
        atom.z = value;
    }
}

#[inline]
pub(crate) fn set_atom_xyz(atom: &mut AtomDatum, xyz: [f64; 3]) {
    set_atom_x(atom, xyz[0]);
    set_atom_y(atom, xyz[1]);
    set_atom_z(atom, xyz[2]);
}
