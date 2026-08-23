#ifndef READCON_DB_MPI_H
#define READCON_DB_MPI_H

/*
 * Optional MPI helper. Not linked into libreadcon_db: include this from a
 * translation unit that already has MPI (LAMMPS pair/fix, mpi4py C ext,
 * a host that passed its sub-communicator).
 *
 * Contract:
 *   - `comm` is the caller's communicator. LAMMPS uses lmp->world / a
 *     sub-comm, not the process-wide world handle. mpi4py already
 *     initialized MPI and owns a Comm (often a Split/Dup).
 *   - This header never names the process-wide world communicator and
 *     never calls MPI_Init / MPI_Finalize.
 *   - Rank `root` of `comm` opens the corpus MDB_RDONLY and packs RCSO;
 *     every other rank on `comm` receives the blob and never opens LMDB.
 */

#include "readcon-db.h"

#include <mpi.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef RKRDB_BCAST_PACK_CAP
#define RKRDB_BCAST_PACK_CAP (1 << 20)
#endif

/**
 * Pack one frame on `root` of `comm` and MPI_Bcast the RCSO blob on `comm`.
 *
 * The host must already have initialized MPI. `comm` is whatever the host
 * already uses for this job (LAMMPS world/subcomm, a Dup/Split). Pass that
 * handle; do not substitute the process-wide world communicator.
 *
 * On success: *out_buf is malloc'd (caller free()), *out_nbytes is the
 * byte count, return RKRDB_OK. On failure every rank returns a negative
 * RKRDB_* code and *out_buf is NULL.
 */
static inline int rkrdb_bcast_packed_frame(MPI_Comm comm, int root,
                                           const char *corpus_dir,
                                           uint64_t traj_id, uint32_t frame_idx,
                                           uint8_t **out_buf, int *out_nbytes) {
  int rank = 0;
  int nbytes = 0;
  uint8_t *buf = NULL;
  int have_buf = 0;
  int all_have = 0;

  if (out_buf == NULL || out_nbytes == NULL)
    return RKRDB_NULL;
  *out_buf = NULL;
  *out_nbytes = 0;

  if (MPI_Comm_rank(comm, &rank) != MPI_SUCCESS)
    return RKRDB_ERR;

  if (rank == root) {
    size_t id = 0;
    if (corpus_dir == NULL)
      nbytes = RKRDB_NULL;
    else if (rkrdb_open_readonly(corpus_dir, &id) != RKRDB_OK)
      nbytes = RKRDB_ERR;
    else {
      buf = (uint8_t *)malloc((size_t)RKRDB_BCAST_PACK_CAP);
      if (buf == NULL) {
        rkrdb_close(id);
        nbytes = RKRDB_ERR;
      } else {
        int n = rkrdb_pack_frame(id, traj_id, frame_idx, buf,
                                 (size_t)RKRDB_BCAST_PACK_CAP);
        rkrdb_close(id);
        if (n < 0) {
          free(buf);
          buf = NULL;
          nbytes = n;
        } else {
          nbytes = n;
        }
      }
    }
  }

  if (MPI_Bcast(&nbytes, 1, MPI_INT, root, comm) != MPI_SUCCESS) {
    free(buf);
    return RKRDB_ERR;
  }
  if (nbytes < 0) {
    free(buf);
    return nbytes;
  }

  if (rank != root) {
    if (nbytes == 0)
      buf = NULL;
    else {
      buf = (uint8_t *)malloc((size_t)nbytes);
    }
  }

  have_buf = (nbytes == 0 || buf != NULL) ? 1 : 0;
  if (MPI_Allreduce(&have_buf, &all_have, 1, MPI_INT, MPI_MIN, comm) !=
      MPI_SUCCESS) {
    free(buf);
    return RKRDB_ERR;
  }
  if (!all_have) {
    free(buf);
    return RKRDB_ERR;
  }

  if (nbytes > 0) {
    if (MPI_Bcast(buf, nbytes, MPI_BYTE, root, comm) != MPI_SUCCESS) {
      free(buf);
      return RKRDB_ERR;
    }
  }

  *out_buf = buf;
  *out_nbytes = nbytes;
  return RKRDB_OK;
}

/**
 * Pack many frames on `root` of `comm` as one RCSB envelope and Bcast
 * that blob on `comm`. One collective for a NEB band / dump window.
 */
static inline int rkrdb_bcast_packed_frames(MPI_Comm comm, int root,
                                            const char *corpus_dir,
                                            const uint64_t *traj_ids,
                                            const uint32_t *frame_idxs,
                                            uint32_t nkeys, uint8_t **out_buf,
                                            int *out_nbytes) {
  int rank = 0;
  int nbytes = 0;
  uint8_t *buf = NULL;
  int have_buf = 0;
  int all_have = 0;

  if (out_buf == NULL || out_nbytes == NULL)
    return RKRDB_NULL;
  *out_buf = NULL;
  *out_nbytes = 0;

  if (MPI_Comm_rank(comm, &rank) != MPI_SUCCESS)
    return RKRDB_ERR;

  if (rank == root) {
    size_t id = 0;
    if (corpus_dir == NULL || traj_ids == NULL || frame_idxs == NULL || nkeys == 0)
      nbytes = RKRDB_NULL;
    else if (rkrdb_open_readonly(corpus_dir, &id) != RKRDB_OK)
      nbytes = RKRDB_ERR;
    else {
      int need = rkrdb_pack_frames(id, traj_ids, frame_idxs, nkeys, NULL, 0);
      if (need < 0)
        nbytes = need;
      else {
        buf = (uint8_t *)malloc((size_t)need);
        if (buf == NULL)
          nbytes = RKRDB_ERR;
        else {
          int n = rkrdb_pack_frames(id, traj_ids, frame_idxs, nkeys, buf,
                                    (size_t)need);
          if (n < 0) {
            free(buf);
            buf = NULL;
            nbytes = n;
          } else {
            nbytes = n;
          }
        }
      }
      rkrdb_close(id);
    }
  }

  if (MPI_Bcast(&nbytes, 1, MPI_INT, root, comm) != MPI_SUCCESS) {
    free(buf);
    return RKRDB_ERR;
  }
  if (nbytes < 0) {
    free(buf);
    return nbytes;
  }

  if (rank != root) {
    if (nbytes == 0)
      buf = NULL;
    else
      buf = (uint8_t *)malloc((size_t)nbytes);
  }

  have_buf = (nbytes == 0 || buf != NULL) ? 1 : 0;
  if (MPI_Allreduce(&have_buf, &all_have, 1, MPI_INT, MPI_MIN, comm) !=
      MPI_SUCCESS) {
    free(buf);
    return RKRDB_ERR;
  }
  if (!all_have) {
    free(buf);
    return RKRDB_ERR;
  }

  if (nbytes > 0) {
    if (MPI_Bcast(buf, nbytes, MPI_BYTE, root, comm) != MPI_SUCCESS) {
      free(buf);
      return RKRDB_ERR;
    }
  }

  *out_buf = buf;
  *out_nbytes = nbytes;
  return RKRDB_OK;
}

/**
 * Same helper for a Fortran INTEGER communicator (`MPI_Fint`).
 * Converts with MPI_Comm_f2c, then broadcasts on that handle.
 */
static inline int rkrdb_bcast_packed_frame_f(MPI_Fint comm_f, int root,
                                             const char *corpus_dir,
                                             uint64_t traj_id,
                                             uint32_t frame_idx,
                                             uint8_t **out_buf,
                                             int *out_nbytes) {
  return rkrdb_bcast_packed_frame(MPI_Comm_f2c(comm_f), root, corpus_dir,
                                  traj_id, frame_idx, out_buf, out_nbytes);
}

static inline int rkrdb_bcast_packed_frames_f(MPI_Fint comm_f, int root,
                                              const char *corpus_dir,
                                              const uint64_t *traj_ids,
                                              const uint32_t *frame_idxs,
                                              uint32_t nkeys, uint8_t **out_buf,
                                              int *out_nbytes) {
  return rkrdb_bcast_packed_frames(MPI_Comm_f2c(comm_f), root, corpus_dir,
                                   traj_ids, frame_idxs, nkeys, out_buf,
                                   out_nbytes);
}

#ifdef __cplusplus
} /* extern "C" */

namespace readcon_db {

inline int bcast_packed_frame(MPI_Comm comm, int root, const char *corpus_dir,
                              uint64_t traj_id, uint32_t frame_idx,
                              uint8_t **out_buf, int *out_nbytes) {
  return rkrdb_bcast_packed_frame(comm, root, corpus_dir, traj_id, frame_idx,
                                  out_buf, out_nbytes);
}

inline int bcast_packed_frames(MPI_Comm comm, int root, const char *corpus_dir,
                               const uint64_t *traj_ids,
                               const uint32_t *frame_idxs, uint32_t nkeys,
                               uint8_t **out_buf, int *out_nbytes) {
  return rkrdb_bcast_packed_frames(comm, root, corpus_dir, traj_ids, frame_idxs,
                                   nkeys, out_buf, out_nbytes);
}

} // namespace readcon_db
#endif /* __cplusplus */

#endif /* READCON_DB_MPI_H */
