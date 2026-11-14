//! Coordinate access for `AtomDatum`.
//!
//! readcon-core 0.23 stores coordinates behind `x()` / `set_x()`.

use readcon_core::types::AtomDatum;

#[inline]
pub(crate) fn atom_x(atom: &AtomDatum) -> f64 {
    atom.x()
}

#[inline]
pub(crate) fn atom_y(atom: &AtomDatum) -> f64 {
    atom.y()
}

#[inline]
pub(crate) fn atom_z(atom: &AtomDatum) -> f64 {
    atom.z()
}

#[inline]
pub(crate) fn atom_xyz(atom: &AtomDatum) -> [f64; 3] {
    [atom_x(atom), atom_y(atom), atom_z(atom)]
}

#[cfg(test)]
#[inline]
pub(crate) fn set_atom_x(atom: &mut AtomDatum, value: f64) {
    atom.set_x(value);
}

#[cfg(test)]
#[inline]
pub(crate) fn set_atom_z(atom: &mut AtomDatum, value: f64) {
    atom.set_z(value);
}
