//! Generated CI-NEB band: Ar adatom hop between fcc and hcp hollows on
//! an LJ fcc(111) slab. Endpoints are relaxed in the potential; intermediate
//! images move under the nudged force. Stored forces are the true potential
//! forces, which is what a CON frame of an image carries.

const SIGMA: f64 = 3.405;
const EPS: f64 = 0.1;
const RC_FACTOR: f64 = 2.5;

pub struct Band {
    pub symbols: Vec<String>,
    pub masses: Vec<f64>,
    pub fixed: Vec<[bool; 3]>,
    pub cell: [f64; 3],
    pub pos: Vec<Vec<f64>>,
    pub frc: Vec<Vec<f64>>,
    pub energy: Vec<f64>,
    pub neb_fmax: f64,
    pub endpoint_fmax: f64,
    pub relax_steps: u32,
    pub neb_steps: u32,
    pub k_spring: f64,
}

struct Geom {
    natoms: usize,
    fixed: Vec<bool>,
    cell: [f64; 3],
    rc2: f64,
    vshift: f64,
}

fn lj_pair(r2: f64, g: &Geom) -> (f64, f64) {
    if r2 >= g.rc2 || r2 < 1e-12 {
        return (0.0, 0.0);
    }
    let sr2 = SIGMA * SIGMA / r2;
    let sr6 = sr2 * sr2 * sr2;
    let sr12 = sr6 * sr6;
    let e = 4.0 * EPS * (sr12 - sr6) - g.vshift;
    // dV/dr * (1/r) so the caller multiplies by r_vec: F_i += pref * r_vec
    // F = 24 eps (2 sr12 - sr6) * r_vec / r^2
    let pref = 24.0 * EPS * (2.0 * sr12 - sr6) / r2;
    (e, pref)
}

fn energy_forces(pos: &[f64], g: &Geom) -> (f64, Vec<f64>) {
    let n = g.natoms;
    let mut f = vec![0.0; n * 3];
    let mut e = 0.0;
    for i in 0..n {
        for j in (i + 1)..n {
            let mut dx = pos[i * 3] - pos[j * 3];
            let mut dy = pos[i * 3 + 1] - pos[j * 3 + 1];
            let dz = pos[i * 3 + 2] - pos[j * 3 + 2];
            dx -= g.cell[0] * (dx / g.cell[0]).round();
            dy -= g.cell[1] * (dy / g.cell[1]).round();
            let r2 = dx * dx + dy * dy + dz * dz;
            let (de, pref) = lj_pair(r2, g);
            if de == 0.0 && pref == 0.0 {
                continue;
            }
            e += de;
            let fx = pref * dx;
            let fy = pref * dy;
            let fz = pref * dz;
            f[i * 3] += fx;
            f[i * 3 + 1] += fy;
            f[i * 3 + 2] += fz;
            f[j * 3] -= fx;
            f[j * 3 + 1] -= fy;
            f[j * 3 + 2] -= fz;
        }
    }
    for i in 0..n {
        if g.fixed[i] {
            f[i * 3] = 0.0;
            f[i * 3 + 1] = 0.0;
            f[i * 3 + 2] = 0.0;
        }
    }
    (e, f)
}

fn fmax(f: &[f64]) -> f64 {
    let mut m = 0.0;
    for c in f.chunks_exact(3) {
        let n = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        if n > m {
            m = n;
        }
    }
    m
}

fn fire(pos: &mut [f64], g: &Geom, max_steps: u32, tol: f64) -> (u32, f64, f64, Vec<f64>) {
    let n = pos.len();
    let mut vel = vec![0.0; n];
    let mut dt = 0.05;
    let dt_max = 0.5;
    let mut alpha = 0.1;
    let mut npos = 0i32;
    let mut last_f = vec![0.0; n];
    let mut energy = 0.0;
    let mut steps = 0u32;
    for step in 0..max_steps {
        steps = step + 1;
        let (e, f) = energy_forces(pos, g);
        energy = e;
        last_f = f;
        if fmax(&last_f) < tol {
            break;
        }
        let mut power = 0.0;
        let mut fnorm = 0.0;
        let mut vnorm = 0.0;
        for i in 0..n {
            power += vel[i] * last_f[i];
            fnorm += last_f[i] * last_f[i];
            vnorm += vel[i] * vel[i];
        }
        fnorm = fnorm.sqrt();
        vnorm = vnorm.sqrt();
        if power > 0.0 {
            if fnorm > 0.0 {
                for i in 0..n {
                    vel[i] = (1.0 - alpha) * vel[i] + alpha * last_f[i] / fnorm * vnorm;
                }
            }
            if npos > 5 {
                dt = (dt * 1.1_f64).min(dt_max);
                alpha *= 0.99;
            }
            npos += 1;
        } else {
            vel.fill(0.0);
            alpha = 0.1;
            dt *= 0.5;
            npos = 0;
        }
        for i in 0..n {
            vel[i] += dt * last_f[i];
            pos[i] += dt * vel[i];
        }
    }
    let fm = fmax(&last_f);
    (steps, fm, energy, last_f)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

fn tangent(prev: &[f64], cur: &[f64], next: &[f64], ep: f64, ec: f64, en: f64) -> Vec<f64> {
    let dp = sub(next, cur);
    let dm = sub(cur, prev);
    if en > ec && ec > ep {
        dp
    } else if en < ec && ec < ep {
        dm
    } else {
        let dep = (en - ec).abs();
        let dem = (ec - ep).abs();
        if en > ep {
            dp.iter()
                .zip(&dm)
                .map(|(p, m)| dep * p + dem * m)
                .collect()
        } else {
            dp.iter()
                .zip(&dm)
                .map(|(p, m)| dem * p + dep * m)
                .collect()
        }
    }
}

fn neb_forces(
    images: &[Vec<f64>],
    true_f: &[Vec<f64>],
    energy: &[f64],
    g: &Geom,
    k: f64,
) -> Vec<Vec<f64>> {
    let m = images.len();
    let mut out = vec![vec![0.0; g.natoms * 3]; m];
    let mut highest = 1usize;
    for i in 2..m - 1 {
        if energy[i] > energy[highest] {
            highest = i;
        }
    }
    for i in 1..m - 1 {
        let tau = tangent(
            &images[i - 1],
            &images[i],
            &images[i + 1],
            energy[i - 1],
            energy[i],
            energy[i + 1],
        );
        let tn = norm(&tau).max(1e-30);
        let hat: Vec<f64> = tau.iter().map(|x| x / tn).collect();
        let ft = &true_f[i];
        let parallel = dot(ft, &hat);
        if i == highest {
            for c in 0..hat.len() {
                out[i][c] = ft[c] - 2.0 * parallel * hat[c];
            }
        } else {
            let d1 = norm(&sub(&images[i], &images[i - 1]));
            let d2 = norm(&sub(&images[i + 1], &images[i]));
            let spring = k * (d2 - d1);
            for c in 0..hat.len() {
                out[i][c] = ft[c] - parallel * hat[c] + spring * hat[c];
            }
        }
        for a in 0..g.natoms {
            if g.fixed[a] {
                out[i][a * 3] = 0.0;
                out[i][a * 3 + 1] = 0.0;
                out[i][a * 3 + 2] = 0.0;
            }
        }
    }
    out
}

fn fire_band(
    images: &mut [Vec<f64>],
    g: &Geom,
    k: f64,
    max_steps: u32,
    tol: f64,
) -> (u32, f64, Vec<f64>, Vec<Vec<f64>>) {
    let m = images.len();
    let n = g.natoms * 3;
    let mut vel = vec![vec![0.0; n]; m];
    let mut dt = 0.02;
    let mut alpha = 0.1;
    let mut npos = 0i32;
    let mut steps = 0u32;
    let mut last_fm = f64::MAX;
    let mut energies = vec![0.0; m];
    let mut true_f = vec![vec![0.0; n]; m];
    for step in 0..max_steps {
        steps = step + 1;
        for i in 0..m {
            let (e, f) = energy_forces(&images[i], g);
            energies[i] = e;
            true_f[i] = f;
        }
        let nf = neb_forces(images, &true_f, &energies, g, k);
        last_fm = nf[1..m - 1].iter().map(|f| fmax(f)).fold(0.0, f64::max);
        if last_fm < tol {
            break;
        }
        let mut power = 0.0;
        let mut fnorm = 0.0;
        let mut vnorm = 0.0;
        for i in 1..m - 1 {
            for c in 0..n {
                power += vel[i][c] * nf[i][c];
                fnorm += nf[i][c] * nf[i][c];
                vnorm += vel[i][c] * vel[i][c];
            }
        }
        fnorm = fnorm.sqrt();
        vnorm = vnorm.sqrt();
        if power > 0.0 && fnorm > 0.0 {
            for i in 1..m - 1 {
                for c in 0..n {
                    vel[i][c] = (1.0 - alpha) * vel[i][c] + alpha * nf[i][c] / fnorm * vnorm;
                }
            }
            if npos > 5 {
                dt = (dt * 1.1_f64).min(0.2);
                alpha *= 0.99;
            }
            npos += 1;
        } else {
            for v in vel.iter_mut() {
                v.fill(0.0);
            }
            alpha = 0.1;
            dt *= 0.5;
            npos = 0;
        }
        for i in 1..m - 1 {
            for c in 0..n {
                vel[i][c] += dt * nf[i][c];
                images[i][c] += dt * vel[i][c];
            }
        }
    }
    (steps, last_fm, energies, true_f)
}

pub fn generate() -> Band {
    let a_nn = SIGMA * 2f64.powf(1.0 / 6.0);
    let row_h = a_nn * 3f64.sqrt() / 2.0;
    let layer_dz = a_nn * (6f64.sqrt()) / 3.0;
    let nx = 6i32;
    let ny = 6i32;
    let nlayers = 4i32;
    let shifts = [
        (0.0, 0.0),
        (a_nn / 2.0, a_nn * 3f64.sqrt() / 6.0),
        (0.0, a_nn * 3f64.sqrt() / 3.0),
    ];
    let mut pos = Vec::new();
    let mut fixed = Vec::new();
    for layer in 0..nlayers {
        let (sx, sy) = shifts[(layer as usize) % 3];
        for j in 0..ny {
            for i in 0..nx {
                let x = i as f64 * a_nn + sx;
                let y = j as f64 * row_h + sy;
                let z = layer as f64 * layer_dz;
                pos.extend_from_slice(&[x, y, z]);
                let fix = layer < 2;
                fixed.push(fix);
            }
        }
    }
    let z_top = (nlayers - 1) as f64 * layer_dz;
    let i = nx / 2;
    let j = ny / 2;
    let (bx, by) = shifts[1];
    let (cx, cy) = shifts[2];
    let x_fcc = i as f64 * a_nn + bx;
    let y_fcc = j as f64 * row_h + by;
    let x_hcp = i as f64 * a_nn + cx;
    let y_hcp = j as f64 * row_h + cy;
    let z_ad = z_top + layer_dz;
    pos.extend_from_slice(&[x_fcc, y_fcc, z_ad]);
    fixed.push(false);
    let natoms = fixed.len();
    let cell = [
        nx as f64 * a_nn,
        ny as f64 * row_h,
        z_top + 18.0,
    ];
    let rc = RC_FACTOR * SIGMA;
    let src6 = (SIGMA / rc).powi(6);
    let g = Geom {
        natoms,
        fixed: fixed.clone(),
        cell,
        rc2: rc * rc,
        vshift: 4.0 * EPS * (src6 * src6 - src6),
    };

    let (relax_steps, endpoint_fmax, _, _) = fire(&mut pos, &g, 300, 0.02);
    let mut product = pos.clone();
    let ad = (natoms - 1) * 3;
    product[ad] = x_hcp;
    product[ad + 1] = y_hcp;
    product[ad + 2] = z_ad;
    let (steps_p, fm_p, _, _) = fire(&mut product, &g, 300, 0.02);
    let relax_steps = relax_steps + steps_p;
    let endpoint_fmax = endpoint_fmax.max(fm_p);

    let n_images = 11usize;
    let mut images = Vec::with_capacity(n_images);
    for t in 0..n_images {
        let s = t as f64 / (n_images - 1) as f64;
        let mut im = vec![0.0; natoms * 3];
        for c in 0..im.len() {
            im[c] = (1.0 - s) * pos[c] + s * product[c];
        }
        images.push(im);
    }
    let k = 0.5;
    let (neb_steps, neb_fmax, _, _) = fire_band(&mut images, &g, k, 250, 0.05);
    let mut energy = Vec::with_capacity(n_images);
    let mut frc = Vec::with_capacity(n_images);
    for im in &images {
        let (e, f) = energy_forces(im, &g);
        energy.push(e);
        frc.push(f);
    }
    let symbols = vec!["Ar".to_string(); natoms];
    let masses = vec![39.948; natoms];
    let fixed_axes = fixed
        .into_iter()
        .map(|f| if f { [true; 3] } else { [false; 3] })
        .collect();
    Band {
        symbols,
        masses,
        fixed: fixed_axes,
        cell,
        pos: images,
        frc,
        energy,
        neb_fmax,
        endpoint_fmax,
        relax_steps,
        neb_steps,
        k_spring: k,
    }
}
