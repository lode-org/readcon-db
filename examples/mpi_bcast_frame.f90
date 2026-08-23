! Rank 0 of the caller INTEGER communicator packs RCSO; every rank
! MPI_Bcast on that same handle. LAMMPS Fortran already has this integer
! (world / sub-comm). Never MPI_Init if the host did; never substitute
! MPI_COMM_WORLD for the handle the host passed.
!
!   mpifort -Iinclude examples/mpi_bcast_frame.f90 -lreadcon_db
program mpi_bcast_frame
  use, intrinsic :: iso_c_binding
  use, intrinsic :: iso_fortran_env, only: error_unit
  use mpi
  use readcon_db
  implicit none
  integer :: ierr, rank, comm, already, we_inited
  integer(c_size_t) :: id, buflen
  integer(c_int64_t) :: traj
  integer(c_int32_t) :: frame, natoms
  integer(c_int) :: status
  integer :: nbytes, argc
  integer(c_int8_t), allocatable :: buf(:)
  real(c_double) :: xyz(3 * 4096)
  character(len=512) :: corpus, arg

  call MPI_Initialized(already, ierr)
  we_inited = 0
  if (already == 0) then
    call MPI_Init(ierr)
    we_inited = 1
  end if
  ! Host-owned comm. Standalone: Dup the process-wide handle so the
  ! broadcast uses a caller comm. LAMMPS passes its world/sub-comm here.
  call MPI_Comm_dup(MPI_COMM_WORLD, comm, ierr)
  call MPI_Comm_rank(comm, rank, ierr)

  argc = command_argument_count()
  if (argc < 1) then
    if (rank == 0) write (error_unit, '(a)') &
      'usage: mpi_bcast_frame <corpus_dir> [traj] [frame]'
    call MPI_Comm_free(comm, ierr)
    if (we_inited == 1) call MPI_Finalize(ierr)
    stop 1
  end if
  call get_command_argument(1, corpus)
  traj = 1_c_int64_t
  frame = 0_c_int32_t
  if (argc >= 2) then
    call get_command_argument(2, arg)
    read (arg, *) traj
  end if
  if (argc >= 3) then
    call get_command_argument(3, arg)
    read (arg, *) frame
  end if

  buflen = int(2**20, c_size_t)
  allocate (buf(buflen))
  nbytes = 0
  if (rank == 0) then
    call db_open_readonly(trim(corpus), id, status)
    if (status /= rkrdb_ok) then
      write (error_unit, '(a)') 'open_readonly failed'
      call MPI_Abort(comm, 2, ierr)
    end if
    call db_pack_frame(id, traj, frame, buf, buflen, nbytes, status)
    call db_close(id, status)
    if (status /= rkrdb_ok .or. nbytes <= 0) then
      write (error_unit, '(a)') 'pack_frame failed'
      call MPI_Abort(comm, 3, ierr)
    end if
  end if
  call MPI_Bcast(nbytes, 1, MPI_INTEGER, 0, comm, ierr)
  call MPI_Bcast(buf, nbytes, MPI_BYTE, 0, comm, ierr)
  call db_unpack_positions(buf, int(nbytes, c_size_t), xyz, &
                           4096_c_int32_t, natoms, status)
  if (status /= rkrdb_ok) then
    write (error_unit, '(a,i0,a)') 'rank ', rank, ' unpack failed'
    call MPI_Comm_free(comm, ierr)
    if (we_inited == 1) call MPI_Finalize(ierr)
    stop 4
  end if
  ! Batched pack (RCSB) on the same caller comm. Two keys.
  if (rank == 0) then
    call db_open_readonly(trim(corpus), id, status)
    if (status /= rkrdb_ok) then
      write (error_unit, '(a)') 'open_readonly failed'
      call MPI_Abort(comm, 2, ierr)
    end if
    call db_pack_frames(id, [traj, traj], [frame, frame], 2_c_int32_t, &
                        buf, buflen, nbytes, status)
    call db_close(id, status)
    if (status /= rkrdb_ok .or. nbytes <= 0) then
      write (error_unit, '(a)') 'pack_frames failed'
      call MPI_Abort(comm, 3, ierr)
    end if
  end if
  call MPI_Bcast(nbytes, 1, MPI_INTEGER, 0, comm, ierr)
  call MPI_Bcast(buf, nbytes, MPI_BYTE, 0, comm, ierr)
  call db_unpack_batch_item(buf, int(nbytes, c_size_t), 0_c_int32_t, xyz, &
                            4096_c_int32_t, natoms, status)
  if (status /= rkrdb_ok) then
    write (error_unit, '(a,i0,a)') 'rank ', rank, ' unpack failed'
    call MPI_Comm_free(comm, ierr)
    if (we_inited == 1) call MPI_Finalize(ierr)
    stop 4
  end if
  if (rank == 0) then
    write (*, '(a,i0,a,i0,a,f0.4,a,f0.4,a,f0.4,a)') &
      'bcast ', nbytes, ' bytes, natoms=', natoms, &
      ' xyz0=(', xyz(1), ',', xyz(2), ',', xyz(3), ')'
  end if
  deallocate (buf)
  call MPI_Comm_free(comm, ierr)
  if (we_inited == 1) call MPI_Finalize(ierr)
end program
