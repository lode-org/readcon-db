#ifndef READCON_DB_H
#define READCON_DB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#include <stdexcept>
extern "C" {
#endif

#define RKRDB_OK 0
#define RKRDB_ERR -1
#define RKRDB_NOT_FOUND -2
#define RKRDB_NULL -3

int rkrdb_open(const char *path, size_t *out_id);
/** Existing corpus, MDB_RDONLY. No mkdir. */
int rkrdb_open_readonly(const char *path, size_t *out_id);
/**
 * Pack one frame as RCSO for a unidirectional MPI_Bcast.
 * Rank 0: open_readonly + pack. Others: never open the env; unpack after Bcast.
 * Returns byte count (>=0) or an error code.
 */
int rkrdb_pack_frame(size_t id, uint64_t traj_id, uint32_t frame_idx, uint8_t *buf,
                     size_t buflen);
/** Decode RCSO positions. No corpus handle. */
int rkrdb_unpack_positions(const uint8_t *buf, size_t buflen, double *out_xyz,
                           uint32_t capacity_atoms, uint32_t *out_natoms);
int rkrdb_close(size_t id);
int rkrdb_last_error(size_t id, char *buf, size_t buflen);
int rkrdb_append_trajectory(size_t id, uint64_t traj_id, const char *path, uint32_t *out_n_frames);
/** Create the trajectory or append CON frames after the live count. */
int rkrdb_extend_trajectory(size_t id, uint64_t traj_id, const char *path, uint32_t *out_n_frames);
int rkrdb_select_basic(size_t id, int64_t traj_id, const char *symbol, uint32_t natoms_min,
                       uint32_t natoms_max, uint32_t limit);
int rkrdb_select_hash(size_t id, const uint8_t *hash16);
/* flags: bit0=forces, bit1=velocities, bit2=energy present; use_energy_range!=0 applies energy_min/max */
int rkrdb_select_meta(size_t id, int64_t traj_id, const char *symbol, uint32_t natoms_min,
                      uint32_t natoms_max, double energy_min, double energy_max,
                      int use_energy_range, uint32_t flags, uint32_t limit);
int rkrdb_reindex(size_t id);
int rkrdb_select_campaign(size_t id, int64_t traj_id, const char *symbol, uint32_t natoms_min,
                          uint32_t natoms_max, const char *formula, double energy_min,
                          double energy_max, int use_energy_range, double fmax_min, double fmax_max,
                          int use_fmax_range, const char *elem_sym, uint32_t elem_count,
                          int elem_exact, uint32_t flags, uint32_t limit);
int rkrdb_result_count(size_t id);
int rkrdb_result_key(size_t id, size_t i, uint64_t *out_traj, uint32_t *out_frame);
int rkrdb_frame_hash(size_t id, uint64_t traj_id, uint32_t frame_idx, uint8_t *out_hash16);
int rkrdb_get_frame_text(size_t id, uint64_t traj_id, uint32_t frame_idx, char *buf, size_t buflen);
/** Parsed CON frame (readcon-core RKRConFrame*). Free with free_rkr_frame. */
void *rkrdb_get_frame(size_t id, uint64_t traj_id, uint32_t frame_idx);
/** Canonical multiset formula (Cu:2|H:2) via core index_proj; NUL-terminated into buf. */
int rkrdb_frame_formula(size_t id, uint64_t traj_id, uint32_t frame_idx, char *buf, size_t buflen);
/** Opt-in RCSO cook from CON text (frames stays authority). */
int rkrdb_cook_frame(size_t id, uint64_t traj_id, uint32_t frame_idx);
int rkrdb_delete_cooked(size_t id, uint64_t traj_id, uint32_t frame_idx);
/** 1 = valid cooked, 0 = missing/corrupt, negative = error. */
int rkrdb_has_valid_cooked(size_t id, uint64_t traj_id, uint32_t frame_idx);
int rkrdb_recook_all(size_t id);
/** Prefer frames_soa positions; fallback parse CON. out_xyz holds N*3 doubles. */
int rkrdb_get_positions(size_t id, uint64_t traj_id, uint32_t frame_idx, double *out_xyz,
                        uint32_t capacity_atoms, uint32_t *out_natoms);
int rkrdb_get_forces(size_t id, uint64_t traj_id, uint32_t frame_idx, double *out_xyz,
                     uint32_t capacity_atoms, uint32_t *out_natoms, uint8_t *out_has_forces);
int rkrdb_xxh3_128(const uint8_t *data, size_t len, uint8_t *out_hash16);

/* --- Observation archive: async fixed-composition ledger of oracle
 * evaluations. Writes land on a dedicated writer thread (append is
 * non-blocking); fetch returns rows in the caller's original atom
 * order. See src/archive.rs. --- */
/** Open/create archive at dir (corpus at dir/observations.rdb). z = per-atom
 *  atomic numbers in caller order (natoms entries), cell3 = orthorhombic box
 *  lengths. Writes opaque handle to out_id. */
int rkrdb_archive_open(const char *dir, const uint32_t *z, uint32_t natoms,
                       const double *cell3, size_t *out_id);
/** Enqueue one row: flat 3*natoms positions/forces (caller order) + energy.
 *  Non-blocking; disk failures counted via rkrdb_archive_dropped. */
int rkrdb_archive_append(size_t id, const double *positions, const double *forces,
                         double energy);
/** Block until every enqueued row is committed or counted dropped. */
int rkrdb_archive_flush(size_t id);
/** Rows committed to the corpus (including prior runs). */
int rkrdb_archive_count(size_t id, uint64_t *out_count);
/** Rows accepted by append, including any still queued (prior runs
 *  included); key cache refreshes on this. */
int rkrdb_archive_appended(size_t id, uint64_t *out_count);
/** Rows the writer thread could not persist. */
int rkrdb_archive_dropped(size_t id, uint64_t *out_count);
/** Fetch committed row `index` in caller atom order; buffers hold 3*natoms
 *  doubles. Flush first for a complete snapshot. */
int rkrdb_archive_fetch(size_t id, uint64_t index, double *positions, double *forces,
                        uint32_t capacity_atoms, double *out_energy);
/** Drain the writer, close the corpus, release the handle. */
int rkrdb_archive_close(size_t id);

#ifdef __cplusplus
} /* extern "C" */

namespace readcon_db {

class Corpus {
  size_t id_ = static_cast<size_t>(-1);

public:
  explicit Corpus(const char *path, bool readonly = false) {
    size_t id = 0;
    int st = readonly ? rkrdb_open_readonly(path, &id) : rkrdb_open(path, &id);
    if (st != RKRDB_OK)
      throw std::runtime_error(readonly ? "rkrdb_open_readonly failed"
                                        : "rkrdb_open failed");
    id_ = id;
  }
  ~Corpus() {
    if (id_ != static_cast<size_t>(-1))
      (void)rkrdb_close(id_);
  }
  Corpus(const Corpus &) = delete;
  Corpus &operator=(const Corpus &) = delete;

  uint32_t append_trajectory(uint64_t traj_id, const char *path) {
    uint32_t n = 0;
    if (rkrdb_append_trajectory(id_, traj_id, path, &n) != RKRDB_OK)
      throw std::runtime_error("append failed");
    return n;
  }

  uint32_t extend_trajectory(uint64_t traj_id, const char *path) {
    uint32_t n = 0;
    if (rkrdb_extend_trajectory(id_, traj_id, path, &n) != RKRDB_OK)
      throw std::runtime_error("extend failed");
    return n;
  }

  int select_basic(int64_t traj_id, const char *symbol, uint32_t nmin, uint32_t nmax,
                   uint32_t limit) {
    return rkrdb_select_basic(id_, traj_id, symbol, nmin, nmax, limit);
  }

  int select_meta(int64_t traj_id, const char *symbol, uint32_t nmin, uint32_t nmax,
                  double emin, double emax, int use_energy, uint32_t flags, uint32_t limit) {
    return rkrdb_select_meta(id_, traj_id, symbol, nmin, nmax, emin, emax, use_energy, flags,
                             limit);
  }

  int result_count() { return rkrdb_result_count(id_); }

  void result_key(size_t i, uint64_t *traj, uint32_t *frame) {
    if (rkrdb_result_key(id_, i, traj, frame) != RKRDB_OK)
      throw std::runtime_error("result_key");
  }

  /// Campaign composition formula for a stored frame (core index_proj encoding).
  std::string frame_formula(uint64_t traj_id, uint32_t frame_idx) {
    char buf[512];
    if (rkrdb_frame_formula(id_, traj_id, frame_idx, buf, sizeof(buf)) != RKRDB_OK)
      throw std::runtime_error("frame_formula");
    return std::string(buf);
  }

  void cook_frame(uint64_t traj_id, uint32_t frame_idx) {
    if (rkrdb_cook_frame(id_, traj_id, frame_idx) != RKRDB_OK)
      throw std::runtime_error("cook_frame");
  }
  void delete_cooked(uint64_t traj_id, uint32_t frame_idx) {
    if (rkrdb_delete_cooked(id_, traj_id, frame_idx) != RKRDB_OK)
      throw std::runtime_error("delete_cooked");
  }
  bool has_valid_cooked(uint64_t traj_id, uint32_t frame_idx) {
    int v = rkrdb_has_valid_cooked(id_, traj_id, frame_idx);
    if (v < 0)
      throw std::runtime_error("has_valid_cooked");
    return v == 1;
  }

  /// RCSO blob for MPI_Bcast. Workers call unpack_positions (no handle).
  int pack_frame(uint64_t traj_id, uint32_t frame_idx, uint8_t *buf, size_t buflen) {
    return rkrdb_pack_frame(id_, traj_id, frame_idx, buf, buflen);
  }

  size_t id() const { return id_; }
};

} // namespace readcon_db
#endif /* __cplusplus */

#endif /* READCON_DB_H */
