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
int rkrdb_close(size_t id);
int rkrdb_last_error(size_t id, char *buf, size_t buflen);
int rkrdb_append_trajectory(size_t id, uint64_t traj_id, const char *path, uint32_t *out_n_frames);
int rkrdb_select_basic(size_t id, int64_t traj_id, const char *symbol, uint32_t natoms_min,
                       uint32_t natoms_max, uint32_t limit);
int rkrdb_select_hash(size_t id, const uint8_t *hash16);
int rkrdb_result_count(size_t id);
int rkrdb_result_key(size_t id, size_t i, uint64_t *out_traj, uint32_t *out_frame);
int rkrdb_frame_hash(size_t id, uint64_t traj_id, uint32_t frame_idx, uint8_t *out_hash16);
int rkrdb_get_frame_text(size_t id, uint64_t traj_id, uint32_t frame_idx, char *buf, size_t buflen);
int rkrdb_xxh3_128(const uint8_t *data, size_t len, uint8_t *out_hash16);

#ifdef __cplusplus
} /* extern "C" */

namespace readcon_db {

class Corpus {
  size_t id_ = static_cast<size_t>(-1);

public:
  explicit Corpus(const char *path) {
    size_t id = 0;
    if (rkrdb_open(path, &id) != RKRDB_OK)
      throw std::runtime_error("rkrdb_open failed");
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

  int select_basic(int64_t traj_id, const char *symbol, uint32_t nmin, uint32_t nmax,
                   uint32_t limit) {
    return rkrdb_select_basic(id_, traj_id, symbol, nmin, nmax, limit);
  }

  int result_count() { return rkrdb_result_count(id_); }

  void result_key(size_t i, uint64_t *traj, uint32_t *frame) {
    if (rkrdb_result_key(id_, i, traj, frame) != RKRDB_OK)
      throw std::runtime_error("result_key");
  }

  size_t id() const { return id_; }
};

} // namespace readcon_db
#endif /* __cplusplus */

#endif /* READCON_DB_H */
