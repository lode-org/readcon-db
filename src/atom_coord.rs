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

#[inline]
pub(crate) fn set_atom_x(atom: &mut AtomDatum, value: f64) {
    atom.set_x(value);
}

#[inline]
pub(crate) fn set_atom_y(atom: &mut AtomDatum, value: f64) {
    atom.set_y(value);
}

#[inline]
pub(crate) fn set_atom_z(atom: &mut AtomDatum, value: f64) {
    atom.set_z(value);
}

#[inline]
pub(crate) fn set_atom_xyz(atom: &mut AtomDatum, xyz: [f64; 3]) {
    set_atom_x(atom, xyz[0]);
    set_atom_y(atom, xyz[1]);
    set_atom_z(atom, xyz[2]);
}
