module readcon_db
  use, intrinsic :: iso_c_binding
  implicit none
  private
  public :: rkrdb_ok, rkrdb_err, db_open, db_close, db_append, db_select_basic, &
            db_result_count, db_result_key, db_frame_hash, db_xxh3_128

  integer(c_int), parameter :: rkrdb_ok = 0
  integer(c_int), parameter :: rkrdb_err = -1

  interface
    function rkrdb_open(path, out_id) bind(C, name="rkrdb_open") result(st)
      import :: c_char, c_int, c_size_t
      character(kind=c_char), intent(in) :: path(*)
      integer(c_size_t), intent(out) :: out_id
      integer(c_int) :: st
    end function
    function rkrdb_close(id) bind(C, name="rkrdb_close") result(st)
      import :: c_int, c_size_t
      integer(c_size_t), value :: id
      integer(c_int) :: st
    end function
    function rkrdb_append_trajectory(id, traj_id, path, out_n) bind(C, name="rkrdb_append_trajectory") result(st)
      import :: c_char, c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      character(kind=c_char), intent(in) :: path(*)
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_select_basic(id, traj_id, symbol, nmin, nmax, limit) bind(C, name="rkrdb_select_basic") result(st)
      import :: c_char, c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      character(kind=c_char), intent(in) :: symbol(*)
      integer(c_int32_t), value :: nmin, nmax, limit
      integer(c_int) :: st
    end function
    function rkrdb_result_count(id) bind(C, name="rkrdb_result_count") result(n)
      import :: c_int, c_size_t
      integer(c_size_t), value :: id
      integer(c_int) :: n
    end function
    function rkrdb_result_key(id, i, out_traj, out_frame) bind(C, name="rkrdb_result_key") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id, i
      integer(c_int64_t), intent(out) :: out_traj
      integer(c_int32_t), intent(out) :: out_frame
      integer(c_int) :: st
    end function
    function rkrdb_frame_hash(id, traj_id, frame_idx, out_hash) bind(C, name="rkrdb_frame_hash") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_int8_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      integer(c_int8_t), intent(out) :: out_hash(*)
      integer(c_int) :: st
    end function
    function rkrdb_xxh3_128(data, n, out_hash) bind(C, name="rkrdb_xxh3_128") result(st)
      import :: c_int, c_size_t, c_int8_t
      integer(c_int8_t), intent(in) :: data(*)
      integer(c_size_t), value :: n
      integer(c_int8_t), intent(out) :: out_hash(*)
      integer(c_int) :: st
    end function
  end interface

contains

  function f_c_string(s) result(c)
    character(len=*), intent(in) :: s
    character(kind=c_char), allocatable :: c(:)
    integer :: i, n
    n = len_trim(s)
    allocate(c(n+1))
    do i = 1, n
      c(i) = s(i:i)
    end do
    c(n+1) = c_null_char
  end function

  subroutine db_open(path, id, status)
    character(len=*), intent(in) :: path
    integer(c_size_t), intent(out) :: id
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cp(:)
    cp = f_c_string(path)
    status = rkrdb_open(cp, id)
  end subroutine

  subroutine db_close(id, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int), intent(out) :: status
    status = rkrdb_close(id)
  end subroutine

  subroutine db_append(id, traj_id, path, n_frames, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    character(len=*), intent(in) :: path
    integer(c_int32_t), intent(out) :: n_frames
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cp(:)
    cp = f_c_string(path)
    status = rkrdb_append_trajectory(id, traj_id, cp, n_frames)
  end subroutine

  subroutine db_select_basic(id, traj_id, symbol, nmin, nmax, limit, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    character(len=*), intent(in) :: symbol
    integer(c_int32_t), intent(in) :: nmin, nmax, limit
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cs(:)
    cs = f_c_string(symbol)
    status = rkrdb_select_basic(id, traj_id, cs, nmin, nmax, limit)
  end subroutine

  function db_result_count(id) result(n)
    integer(c_size_t), intent(in) :: id
    integer(c_int) :: n
    n = rkrdb_result_count(id)
  end function

  subroutine db_result_key(id, i, traj, frame, status)
    integer(c_size_t), intent(in) :: id, i
    integer(c_int64_t), intent(out) :: traj
    integer(c_int32_t), intent(out) :: frame
    integer(c_int), intent(out) :: status
    status = rkrdb_result_key(id, i, traj, frame)
  end subroutine

  subroutine db_frame_hash(id, traj_id, frame_idx, hash16, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    integer(c_int8_t), intent(out) :: hash16(16)
    integer(c_int), intent(out) :: status
    status = rkrdb_frame_hash(id, traj_id, frame_idx, hash16)
  end subroutine

  subroutine db_xxh3_128(data, n, hash16, status)
    integer(c_int8_t), intent(in) :: data(*)
    integer(c_size_t), intent(in) :: n
    integer(c_int8_t), intent(out) :: hash16(16)
    integer(c_int), intent(out) :: status
    status = rkrdb_xxh3_128(data, n, hash16)
  end subroutine

end module readcon_db
