/* mpicc ingest_smoke.c -lreadcon_db; mpiexec -n P ./a.out dir file.con...
 * Calls the MPI_Comm wrapper on a split communicator (not the world handle
 * inside the library) and prints the per-file frame counts on rank 0. */
#include "readcon-db-mpi.h"
#include <stdio.h>

int main(int argc, char **argv) {
  MPI_Init(&argc, &argv);
  MPI_Comm comm;
  MPI_Comm_split(MPI_COMM_WORLD, 0, 0, &comm);
  int rank = 0;
  MPI_Comm_rank(comm, &rank);
  size_t n = (size_t)(argc - 2);
  uint64_t ids[64];
  uint32_t counts[64];
  for (size_t i = 0; i < n && i < 64; i++)
    ids[i] = i + 1;
  int rc = rkrdb_mpi_ingest_paths(comm, argv[1], 0, n, ids,
                                  (const char *const *)&argv[2], 2, counts);
  if (rank == 0) {
    printf("rc=%d", rc);
    for (size_t i = 0; i < n; i++)
      printf(" %u", counts[i]);
    printf("\n");
  }
  MPI_Comm_free(&comm);
  MPI_Finalize();
  return rc == 0 ? 0 : 1;
}
