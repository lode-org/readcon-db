//! Reader for the Zenodo 13741551 proton-in-water configurations.
//! Coordinates and forces are plain XYZ (one frame header, then natoms lines).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct XyzTraj {
    pub natoms: usize,
    pub symbols: Vec<String>,
    pub pos: Vec<Vec<f64>>,
    pub frc: Vec<Vec<f64>>,
    pub energy: Vec<f64>,
}

fn load_xyz(path: &Path) -> (usize, Vec<String>, Vec<Vec<f64>>) {
    let f = File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut lines = BufReader::new(f).lines();
    let mut symbols: Option<Vec<String>> = None;
    let mut frames = Vec::new();
    let mut natoms = 0usize;
    loop {
        let Some(nline) = lines.next() else { break };
        let nline = nline.expect("read");
        let nline = nline.trim();
        if nline.is_empty() {
            continue;
        }
        natoms = nline.parse().unwrap_or_else(|_| panic!("natoms line {nline}"));
        let _comment = lines.next().expect("comment").expect("comment");
        let mut xyz = Vec::with_capacity(natoms * 3);
        let mut syms = Vec::with_capacity(natoms);
        for _ in 0..natoms {
            let line = lines.next().expect("atom").expect("atom");
            let mut p = line.split_whitespace();
            let s = p.next().expect("symbol").to_string();
            let x: f64 = p.next().unwrap().parse().unwrap();
            let y: f64 = p.next().unwrap().parse().unwrap();
            let z: f64 = p.next().unwrap().parse().unwrap();
            syms.push(s);
            xyz.extend_from_slice(&[x, y, z]);
        }
        if let Some(prev) = &symbols {
            assert_eq!(prev, &syms, "species order changed between frames");
        } else {
            symbols = Some(syms);
        }
        frames.push(xyz);
    }
    (natoms, symbols.expect("no frames"), frames)
}

pub fn load(conf: &Path, forces: &Path, energies: &Path) -> XyzTraj {
    let (natoms, symbols, pos) = load_xyz(conf);
    let (n2, symbols2, frc) = load_xyz(forces);
    assert_eq!(natoms, n2);
    assert_eq!(symbols, symbols2);
    assert_eq!(pos.len(), frc.len());
    let energy = std::fs::read_to_string(energies)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .map(|l| l.trim().parse::<f64>().expect("energy"))
        .collect::<Vec<_>>();
    assert_eq!(energy.len(), pos.len(), "energy count");
    XyzTraj {
        natoms,
        symbols,
        pos,
        frc,
        energy,
    }
}

pub fn rmsd_report(pos: &[Vec<f64>], box_l: f64) -> (f64, f64, f64) {
    let n = pos[0].len() / 3;
    let mut cons = Vec::new();
    for w in pos.windows(2) {
        let mut s = 0.0;
        for i in 0..n {
            for k in 0..3 {
                let mut d = w[1][i * 3 + k] - w[0][i * 3 + k];
                d -= box_l * (d / box_l).round();
                s += d * d;
            }
        }
        cons.push((s / n as f64).sqrt());
    }
    cons.sort_by(|a, b| a.total_cmp(b));
    let mean = cons.iter().sum::<f64>() / cons.len() as f64;
    let median = cons[cons.len() / 2];
    let max = cons[cons.len() - 1];
    (mean, median, max)
}
