//! FFI to the zfp 1.0.1 reversible mode and to c-blosc2 2.15.2.
//! Layout of `blosc2_cparams` is checked against the installed headers
//! (sizeof 136, filters at offset 32). A round trip in `self_test` is the
//! runtime check that the Rust struct still matches.

use std::os::raw::{c_int, c_void};

const BLOSC_ZSTD: u8 = 5;
const BLOSC_SHUFFLE: u8 = 1;
const BLOSC_BITSHUFFLE: u8 = 2;
const BLOSC_DELTA: u8 = 3;
const BLOSC_FILTER_BYTEDELTA: u8 = 35;
const BLOSC_FORWARD_COMPAT_SPLIT: i32 = 4;
const ZFP_TYPE_DOUBLE: c_int = 4;

#[repr(C)]
struct Blosc2Cparams {
    compcode: u8,
    compcode_meta: u8,
    clevel: u8,
    use_dict: i32,
    typesize: i32,
    nthreads: i16,
    blocksize: i32,
    splitmode: i32,
    schunk: *mut c_void,
    filters: [u8; 6],
    filters_meta: [u8; 6],
    prefilter: *mut c_void,
    preparams: *mut c_void,
    tuner_params: *mut c_void,
    tuner_id: i32,
    instr_codec: bool,
    codec_params: *mut c_void,
    filter_params: [*mut c_void; 6],
}

#[repr(C)]
struct Blosc2Dparams {
    nthreads: i16,
    schunk: *mut c_void,
    postfilter: *mut c_void,
    postparams: *mut c_void,
}

extern "C" {
    fn blosc2_init();
    fn blosc2_create_cctx(cparams: Blosc2Cparams) -> *mut c_void;
    fn blosc2_create_dctx(dparams: Blosc2Dparams) -> *mut c_void;
    fn blosc2_free_ctx(context: *mut c_void);
    fn blosc2_compress_ctx(
        context: *mut c_void,
        src: *const c_void,
        srcsize: i32,
        dest: *mut c_void,
        destsize: i32,
    ) -> i32;
    fn blosc2_decompress_ctx(
        context: *mut c_void,
        src: *const c_void,
        srcsize: i32,
        dest: *mut c_void,
        destsize: i32,
    ) -> i32;

    fn zfp_field_2d(pointer: *mut c_void, ty: c_int, nx: usize, ny: usize) -> *mut c_void;
    fn zfp_field_3d(
        pointer: *mut c_void,
        ty: c_int,
        nx: usize,
        ny: usize,
        nz: usize,
    ) -> *mut c_void;
    fn zfp_field_free(field: *mut c_void);
    fn zfp_stream_open(stream: *mut c_void) -> *mut c_void;
    fn zfp_stream_close(stream: *mut c_void);
    fn zfp_stream_set_reversible(stream: *mut c_void);
    fn zfp_stream_set_bit_stream(stream: *mut c_void, bs: *mut c_void);
    fn zfp_stream_rewind(stream: *mut c_void);
    fn zfp_stream_maximum_size(stream: *const c_void, field: *const c_void) -> usize;
    fn zfp_compress(stream: *mut c_void, field: *const c_void) -> usize;
    fn zfp_decompress(stream: *mut c_void, field: *mut c_void) -> usize;
    fn stream_open(buffer: *mut c_void, bytes: usize) -> *mut c_void;
    fn stream_close(stream: *mut c_void);
}

fn cparams(filters: [u8; 6], meta: [u8; 6]) -> Blosc2Cparams {
    Blosc2Cparams {
        compcode: BLOSC_ZSTD,
        compcode_meta: 0,
        clevel: 3,
        use_dict: 0,
        typesize: 8,
        nthreads: 1,
        blocksize: 0,
        splitmode: BLOSC_FORWARD_COMPAT_SPLIT,
        schunk: std::ptr::null_mut(),
        filters,
        filters_meta: meta,
        prefilter: std::ptr::null_mut(),
        preparams: std::ptr::null_mut(),
        tuner_params: std::ptr::null_mut(),
        tuner_id: 0,
        instr_codec: false,
        codec_params: std::ptr::null_mut(),
        filter_params: [std::ptr::null_mut(); 6],
    }
}

fn dparams() -> Blosc2Dparams {
    Blosc2Dparams {
        nthreads: 1,
        schunk: std::ptr::null_mut(),
        postfilter: std::ptr::null_mut(),
        postparams: std::ptr::null_mut(),
    }
}

pub struct BloscMode {
    name: &'static str,
    cctx: *mut c_void,
    dctx: *mut c_void,
}

// The contexts are created once and used from the single measurement thread.
unsafe impl Send for BloscMode {}

impl Drop for BloscMode {
    fn drop(&mut self) {
        unsafe {
            if !self.cctx.is_null() {
                blosc2_free_ctx(self.cctx);
            }
            if !self.dctx.is_null() {
                blosc2_free_ctx(self.dctx);
            }
        }
    }
}

impl BloscMode {
    fn new(name: &'static str, filters: [u8; 6], meta: [u8; 6]) -> Self {
        let cctx = unsafe { blosc2_create_cctx(cparams(filters, meta)) };
        let dctx = unsafe { blosc2_create_dctx(dparams()) };
        assert!(!cctx.is_null() && !dctx.is_null(), "blosc2 context");
        Self { name, cctx, dctx }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn compress(&self, raw: &[u8]) -> Vec<u8> {
        assert!(raw.len() <= i32::MAX as usize);
        let dest_len = raw.len() + 64 + raw.len() / 8;
        let mut dest = vec![0u8; dest_len.max(64)];
        let n = unsafe {
            blosc2_compress_ctx(
                self.cctx,
                raw.as_ptr() as *const c_void,
                raw.len() as i32,
                dest.as_mut_ptr() as *mut c_void,
                dest.len() as i32,
            )
        };
        assert!(n > 0, "blosc2 compress {} -> {n}", self.name);
        dest.truncate(n as usize);
        dest
    }

    pub fn decompress(&self, src: &[u8], dst: &mut [u8]) {
        let n = unsafe {
            blosc2_decompress_ctx(
                self.dctx,
                src.as_ptr() as *const c_void,
                src.len() as i32,
                dst.as_mut_ptr() as *mut c_void,
                dst.len() as i32,
            )
        };
        assert_eq!(n, dst.len() as i32, "blosc2 decompress {}", self.name);
    }
}

pub fn blosc_modes() -> Vec<BloscMode> {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe { blosc2_init() });
    let z = [0u8; 6];
    vec![
        BloscMode::new(
            "blosc2-shuffle-zstd3",
            [0, 0, 0, 0, 0, BLOSC_SHUFFLE],
            z,
        ),
        BloscMode::new(
            "blosc2-bitshuffle-zstd3",
            [0, 0, 0, 0, 0, BLOSC_BITSHUFFLE],
            z,
        ),
        // Index 0 runs first: shuffle, then bytedelta on the byte streams.
        BloscMode::new(
            "blosc2-shuffle-bytedelta-zstd3",
            [0, 0, 0, 0, BLOSC_SHUFFLE, BLOSC_FILTER_BYTEDELTA],
            [0, 0, 0, 0, 0, 8],
        ),
        BloscMode::new(
            "blosc2-delta-shuffle-zstd3",
            [0, 0, 0, 0, BLOSC_DELTA, BLOSC_SHUFFLE],
            z,
        ),
    ]
}

fn zfp_compress_field(field: *mut c_void) -> Vec<u8> {
    unsafe {
        let stream = zfp_stream_open(std::ptr::null_mut());
        assert!(!stream.is_null());
        zfp_stream_set_reversible(stream);
        let max = zfp_stream_maximum_size(stream, field).max(64);
        let mut buf = vec![0u8; max];
        let bs = stream_open(buf.as_mut_ptr() as *mut c_void, buf.len());
        assert!(!bs.is_null());
        zfp_stream_set_bit_stream(stream, bs);
        zfp_stream_rewind(stream);
        let n = zfp_compress(stream, field);
        stream_close(bs);
        zfp_stream_close(stream);
        zfp_field_free(field);
        assert!(n > 0, "zfp reversible compress returned 0");
        buf.truncate(n);
        buf
    }
}

fn zfp_decompress_field(field: *mut c_void, src: &[u8]) {
    unsafe {
        let stream = zfp_stream_open(std::ptr::null_mut());
        zfp_stream_set_reversible(stream);
        // The bitstream reads the compressed bytes. zfp does not retain the pointer
        // after zfp_decompress returns, but the buffer must live for the call.
        let mut owned = src.to_vec();
        let bs = stream_open(owned.as_mut_ptr() as *mut c_void, owned.len());
        zfp_stream_set_bit_stream(stream, bs);
        zfp_stream_rewind(stream);
        let n = zfp_decompress(stream, field);
        stream_close(bs);
        zfp_stream_close(stream);
        zfp_field_free(field);
        assert!(n > 0, "zfp reversible decompress returned 0");
    }
}

/// `data` is row-major `f[ny][nx]` with `nx` contiguous. Reversible mode.
pub fn zfp_compress_2d(data: &[f64], nx: usize, ny: usize) -> Vec<u8> {
    assert_eq!(data.len(), nx * ny);
    let field = unsafe {
        zfp_field_2d(
            data.as_ptr() as *mut c_void,
            ZFP_TYPE_DOUBLE,
            nx,
            ny,
        )
    };
    zfp_compress_field(field)
}

pub fn zfp_decompress_2d(src: &[u8], nx: usize, ny: usize) -> Vec<f64> {
    let mut out = vec![0f64; nx * ny];
    let field = unsafe {
        zfp_field_2d(
            out.as_mut_ptr() as *mut c_void,
            ZFP_TYPE_DOUBLE,
            nx,
            ny,
        )
    };
    zfp_decompress_field(field, src);
    out
}

/// `data` is row-major `f[nz][ny][nx]`.
pub fn zfp_compress_3d(data: &[f64], nx: usize, ny: usize, nz: usize) -> Vec<u8> {
    assert_eq!(data.len(), nx * ny * nz);
    let field = unsafe {
        zfp_field_3d(
            data.as_ptr() as *mut c_void,
            ZFP_TYPE_DOUBLE,
            nx,
            ny,
            nz,
        )
    };
    zfp_compress_field(field)
}

pub fn zfp_decompress_3d(src: &[u8], nx: usize, ny: usize, nz: usize) -> Vec<f64> {
    let mut out = vec![0f64; nx * ny * nz];
    let field = unsafe {
        zfp_field_3d(
            out.as_mut_ptr() as *mut c_void,
            ZFP_TYPE_DOUBLE,
            nx,
            ny,
            nz,
        )
    };
    zfp_decompress_field(field, src);
    out
}

pub fn self_test() {
    let raw: Vec<f64> = (0..48).map(|i| (i as f64) * 0.01 + 1.25).collect();
    let bytes: Vec<u8> = raw.iter().flat_map(|x| x.to_le_bytes()).collect();
    for mode in blosc_modes() {
        let c = mode.compress(&bytes);
        let mut back = vec![0u8; bytes.len()];
        mode.decompress(&c, &mut back);
        assert_eq!(back, bytes, "{}", mode.name());
    }
    let c = zfp_compress_2d(&raw, 3, 16);
    let back = zfp_decompress_2d(&c, 3, 16);
    assert_eq!(
        raw.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        back.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    let c3 = zfp_compress_3d(&raw, 3, 4, 4);
    let back3 = zfp_decompress_3d(&c3, 3, 4, 4);
    assert_eq!(
        raw.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        back3.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
}
