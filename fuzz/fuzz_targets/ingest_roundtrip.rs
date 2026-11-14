//! Ingest a generated trajectory and compare the database to readcon-core.

#![no_main]

use libfuzzer_sys::fuzz_target;
use readcon_core::compression::read_file_bytes;
use readcon_core::conbin::{read_owned_f64, write_from_con};
use readcon_core::iterators::frames_from_bytes;
use readcon_core::types::{ConFrame, ConFrameBuilder};
use readcon_core::units::NATURAL_TIME_UNIT;
use readcon_core::writer::{write_frames_to_vec, ConFrameWriter};
use readcon_db::ConCorpus;

struct Input<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Input<'a> {
    fn byte(&mut self) -> u8 {
        let value = self.data.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        value
    }

    fn finite(&mut self) -> f64 {
        match self.byte() & 7 {
            0 => -0.0,
            1 => f64::from_bits(1),
            2 => f64::from_bits(0x8000_0000_0000_0001),
            3 => f64::MIN_POSITIVE,
            4 => f64::MAX,
            5 => -f64::MAX,
            6 => f64::from_bits(0x000f_ffff_ffff_ffff),
            _ => {
                let mut buf = [0u8; 8];
                for slot in &mut buf {
                    *slot = self.byte();
                }
                let value = f64::from_le_bytes(buf);
                if value.is_finite() { value } else { 0.0 }
            }
        }
    }
}

fn symbol(id: u8) -> &'static str {
    match id % 8 {
        0 => "Cu",
        1 => "H",
        2 => "O",
        3 => "He",
        4 => "C",
        5 => "N",
        6 => "Fe",
        _ => "Si",
    }
}

fn frames_from(data: &[u8]) -> Option<Vec<ConFrame>> {
    let mut input = Input { data, at: 0 };
    let n_frames = 1 + usize::from(input.byte() % 3);
    let mut frames = Vec::with_capacity(n_frames);
    for _ in 0..n_frames {
        let mut builder = ConFrameBuilder::new(
            [input.finite(), input.finite(), input.finite()],
            [input.finite(), input.finite(), input.finite()],
        );
        builder
            .set_units(serde_json::json!({
                "length": "angstrom",
                "energy": "eV",
                "mass": "amu",
                "time": NATURAL_TIME_UNIT,
            }))
            .ok()?;
        let atoms = 1 + usize::from(input.byte() % 4);
        let velocities = input.byte() & 1 == 1;
        let mut masses = [None; 8];
        for index in 0..atoms {
            let species = input.byte() % 8;
            let mass = *masses[species as usize].get_or_insert_with(|| input.finite());
            builder.add_atom(
                symbol(species),
                input.finite(),
                input.finite(),
                input.finite(),
                [
                    input.byte() & 1 == 1,
                    input.byte() & 1 == 1,
                    input.byte() & 1 == 1,
                ],
                index as u64,
                mass,
            );
            if velocities {
                builder.with_velocity([input.finite(), input.finite(), input.finite()]);
            }
        }
        frames.push(builder.build().ok()?);
    }
    Some(frames)
}

fn bits(value: f64) -> u64 {
    value.to_bits()
}

fn assert_positions(n_frames: u32, values: &[f64], frames: &[ConFrame]) {
    assert_eq!(n_frames as usize, frames.len());
    let mut expect = Vec::new();
    for frame in frames {
        for atom in &frame.atom_data {
            expect.extend_from_slice(&[atom.x(), atom.y(), atom.z()]);
        }
    }
    assert_eq!(values.len(), expect.len());
    for (got, expect) in values.iter().zip(expect.iter()) {
        assert_eq!(bits(*got), bits(*expect));
    }
}

fn assert_frames(got: &[ConFrame], expect: &[ConFrame]) {
    assert_eq!(got.len(), expect.len());
    for (got, expect) in got.iter().zip(expect.iter()) {
        assert_eq!(got.atom_data.len(), expect.atom_data.len());
        for (got_atom, expect_atom) in got.atom_data.iter().zip(expect.atom_data.iter()) {
            assert_eq!(got_atom.symbol.as_ref(), expect_atom.symbol.as_ref());
            assert_eq!(bits(got_atom.x()), bits(expect_atom.x()));
            assert_eq!(bits(got_atom.y()), bits(expect_atom.y()));
            assert_eq!(bits(got_atom.z()), bits(expect_atom.z()));
            assert_eq!(got_atom.fixed(), expect_atom.fixed());
            assert_eq!(got_atom.atom_id, expect_atom.atom_id);
            match (got_atom.velocity(), expect_atom.velocity()) {
                (Some(got_v), Some(expect_v)) => {
                    for index in 0..3 {
                        assert_eq!(bits(got_v[index]), bits(expect_v[index]));
                    }
                }
                (None, None) => {}
                (got_v, expect_v) => panic!("velocity {got_v:?} vs {expect_v:?}"),
            }
        }
    }
}

fn ingest_text(db: &ConCorpus, traj: u64, text: &str, frames: &[ConFrame]) {
    db.append_trajectory_str(traj, text, "fuzz")
        .expect("ingest");
    let block = db.trajectory_positions(traj).expect("positions");
    assert_positions(block.n_frames, &block.values, frames);
    let keys = db.list_frame_keys().expect("keys");
    let mut stored = Vec::new();
    for key in keys {
        if key.traj_id == traj {
            stored.push(db.get_frame(key).expect("frame"));
        }
    }
    assert_frames(&stored, frames);
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }
    let Some(frames) = frames_from(data) else {
        return;
    };
    let plain = write_frames_to_vec(&frames).expect("write con");
    let text = std::str::from_utf8(&plain).expect("utf8");
    let core = frames_from_bytes(&plain, Some(1)).expect("core parse");
    assert_frames(&core, &frames);

    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let db = ConCorpus::open_with_map_size(dir.path(), 8 * 1024 * 1024).expect("db");
    ingest_text(&db, 1, text, &frames);

    let gz = dir.path().join("fuzz.con.gz");
    {
        let mut writer = ConFrameWriter::from_path_gzip(&gz).expect("gzip");
        for frame in &frames {
            writer.write_frame(frame).expect("gzip frame");
        }
    }
    let inflated = read_file_bytes(&gz).expect("inflate gzip");
    let gz_text = std::str::from_utf8(&inflated).expect("gzip utf8");
    ingest_text(&db, 2, gz_text, &frames);

    let zst = dir.path().join("fuzz.con.zst");
    {
        let mut writer = ConFrameWriter::from_path_zstd(&zst).expect("zstd");
        for frame in &frames {
            writer.write_frame(frame).expect("zstd frame");
        }
    }
    let inflated = read_file_bytes(&zst).expect("inflate zstd");
    let zst_text = std::str::from_utf8(&inflated).expect("zstd utf8");
    ingest_text(&db, 3, zst_text, &frames);

    let con = dir.path().join("fuzz.con");
    let bin = dir.path().join("fuzz.conbin");
    std::fs::write(&con, &plain).expect("con file");
    write_from_con(&con, &bin, false).expect("conbin");
    let (_header, positions, _) = read_owned_f64(&bin, 1).expect("read conbin");
    let block = db.trajectory_positions(1).expect("db positions");
    assert_eq!(positions.len(), block.values.len());
    for (got, expect) in positions.iter().zip(block.values.iter()) {
        assert_eq!(bits(*got), bits(*expect));
    }
});
