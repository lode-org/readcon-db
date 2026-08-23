module readcon_db
  use, intrinsic :: iso_c_binding
  implicit none
  private
  public :: rkrdb_ok, rkrdb_err, db_open, db_open_readonly, db_close, db_append, db_append_units, &
            db_select_basic, &
            db_result_count, db_result_key, db_frame_hash, db_frame_formula, db_xxh3_128, &
            db_get_frame, db_pack_frame, db_pack_frames, db_unpack_positions, &
            db_unpack_batch_nframes, db_unpack_batch_item, db_set_units, db_frame_units, &
            db_h5md_times, db_h5md_shape, db_h5md_positions

  integer(c_int), parameter :: rkrdb_ok = 0
  integer(c_int), parameter :: rkrdb_err = -1

  interface
    function rkrdb_open(path, out_id) bind(C, name="rkrdb_open") result(st)
      import :: c_char, c_int, c_size_t
      character(kind=c_char), intent(in) :: path(*)
      integer(c_size_t), intent(out) :: out_id
      integer(c_int) :: st
    end function
    function rkrdb_open_readonly(path, out_id) bind(C, name="rkrdb_open_readonly") result(st)
      import :: c_char, c_int, c_size_t
      character(kind=c_char), intent(in) :: path(*)
      integer(c_size_t), intent(out) :: out_id
      integer(c_int) :: st
    end function
    function rkrdb_pack_frame(id, traj_id, frame_idx, buf, buflen) bind(C, name="rkrdb_pack_frame") result(n)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_int8_t
      integer(c_size_t), value :: id, buflen
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      integer(c_int8_t), intent(out) :: buf(*)
      integer(c_int) :: n
    end function
    function rkrdb_pack_frames(id, traj_ids, frame_idxs, nkeys, buf, buflen) &
        bind(C, name="rkrdb_pack_frames") result(n)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_int8_t
      integer(c_size_t), value :: id, buflen
      integer(c_int64_t), intent(in) :: traj_ids(*)
      integer(c_int32_t), intent(in) :: frame_idxs(*)
      integer(c_int32_t), value :: nkeys
      integer(c_int8_t), intent(out) :: buf(*)
      integer(c_int) :: n
    end function
    function rkrdb_unpack_batch_nframes(buf, buflen, out_n) &
        bind(C, name="rkrdb_unpack_batch_nframes") result(st)
      import :: c_int, c_size_t, c_int8_t, c_int32_t
      integer(c_int8_t), intent(in) :: buf(*)
      integer(c_size_t), value :: buflen
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_unpack_batch_item(buf, buflen, idx, out_xyz, cap, out_n) &
        bind(C, name="rkrdb_unpack_batch_item") result(st)
      import :: c_int, c_size_t, c_int8_t, c_double, c_int32_t
      integer(c_int8_t), intent(in) :: buf(*)
      integer(c_size_t), value :: buflen
      integer(c_int32_t), value :: idx, cap
      real(c_double), intent(out) :: out_xyz(*)
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_unpack_positions(buf, buflen, out_xyz, cap, out_n) bind(C, name="rkrdb_unpack_positions") result(st)
      import :: c_int, c_size_t, c_int8_t, c_double, c_int32_t
      integer(c_int8_t), intent(in) :: buf(*)
      integer(c_size_t), value :: buflen
      real(c_double), intent(out) :: out_xyz(*)
      integer(c_int32_t), value :: cap
      integer(c_int32_t), intent(out) :: out_n
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
    function rkrdb_append_trajectory_units(id, traj_id, path, units_json, out_n) &
        bind(C, name="rkrdb_append_trajectory_units") result(st)
      import :: c_char, c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      character(kind=c_char), intent(in) :: path(*)
      character(kind=c_char), intent(in) :: units_json(*)
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_set_units(id, traj_id, units_json, out_n) bind(C, name="rkrdb_set_units") result(st)
      import :: c_char, c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      character(kind=c_char), intent(in) :: units_json(*)
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_frame_units(id, traj_id, frame_idx, buf, buflen) &
        bind(C, name="rkrdb_frame_units") result(st)
      import :: c_char, c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id, buflen
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      character(kind=c_char), intent(out) :: buf(*)
      integer(c_int) :: st
    end function
    function rkrdb_h5md_times(id, traj_id, out, cap, out_n) bind(C, name="rkrdb_h5md_times") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_double
      integer(c_size_t), value :: id, cap
      integer(c_int64_t), value :: traj_id
      real(c_double), intent(out) :: out(*)
      integer(c_int32_t), intent(out) :: out_n
      integer(c_int) :: st
    end function
    function rkrdb_h5md_shape(id, traj_id, out_nframes, out_natoms) &
        bind(C, name="rkrdb_h5md_shape") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), intent(out) :: out_nframes, out_natoms
      integer(c_int) :: st
    end function
    function rkrdb_h5md_positions(id, traj_id, out, cap, out_nframes, out_natoms) &
        bind(C, name="rkrdb_h5md_positions") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_double
      integer(c_size_t), value :: id, cap
      integer(c_int64_t), value :: traj_id
      real(c_double), intent(out) :: out(*)
      integer(c_int32_t), intent(out) :: out_nframes, out_natoms
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
    function rkrdb_get_frame(id, traj_id, frame_idx) bind(C, name="rkrdb_get_frame") result(p)
      import :: c_size_t, c_int64_t, c_int32_t, c_ptr
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      type(c_ptr) :: p
    end function
    function rkrdb_frame_formula(id, traj_id, frame_idx, buf, buflen) bind(C, name="rkrdb_frame_formula") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t, c_char
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      character(kind=c_char), intent(out) :: buf(*)
      integer(c_size_t), value :: buflen
      integer(c_int) :: st
    end function

    function rkrdb_cook_frame(id, traj_id, frame_idx) bind(C, name="rkrdb_cook_frame") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      integer(c_int) :: st
    end function
    function rkrdb_delete_cooked(id, traj_id, frame_idx) bind(C, name="rkrdb_delete_cooked") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      integer(c_int) :: st
    end function
    function rkrdb_has_valid_cooked(id, traj_id, frame_idx) bind(C, name="rkrdb_has_valid_cooked") result(st)
      import :: c_int, c_size_t, c_int64_t, c_int32_t
      integer(c_size_t), value :: id
      integer(c_int64_t), value :: traj_id
      integer(c_int32_t), value :: frame_idx
      integer(c_int) :: st
    end function
    function rkrdb_recook_all(id) bind(C, name="rkrdb_recook_all") result(st)
      import :: c_int, c_size_t
      integer(c_size_t), value :: id
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

  subroutine db_open_readonly(path, id, status)
    character(len=*), intent(in) :: path
    integer(c_size_t), intent(out) :: id
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cp(:)
    cp = f_c_string(path)
    status = rkrdb_open_readonly(cp, id)
  end subroutine

  subroutine db_pack_frame(id, traj_id, frame_idx, buf, buflen, n, status)
    integer(c_size_t), intent(in) :: id, buflen
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    integer(c_int8_t), intent(out) :: buf(*)
    integer, intent(out) :: n
    integer(c_int), intent(out) :: status
    integer(c_int) :: rc
    rc = rkrdb_pack_frame(id, traj_id, frame_idx, buf, buflen)
    if (rc < 0) then
      status = rc
      n = 0
    else
      status = rkrdb_ok
      n = int(rc)
    end if
  end subroutine

  subroutine db_pack_frames(id, traj_ids, frame_idxs, nkeys, buf, buflen, n, status)
    integer(c_size_t), intent(in) :: id, buflen
    integer(c_int64_t), intent(in) :: traj_ids(*)
    integer(c_int32_t), intent(in) :: frame_idxs(*)
    integer(c_int32_t), intent(in) :: nkeys
    integer(c_int8_t), intent(out) :: buf(*)
    integer, intent(out) :: n
    integer(c_int), intent(out) :: status
    integer(c_int) :: rc
    rc = rkrdb_pack_frames(id, traj_ids, frame_idxs, nkeys, buf, buflen)
    if (rc < 0) then
      status = rc
      n = 0
    else
      status = rkrdb_ok
      n = int(rc)
    end if
  end subroutine

  subroutine db_unpack_batch_nframes(buf, buflen, nframes, status)
    integer(c_int8_t), intent(in) :: buf(*)
    integer(c_size_t), intent(in) :: buflen
    integer(c_int32_t), intent(out) :: nframes
    integer(c_int), intent(out) :: status
    status = rkrdb_unpack_batch_nframes(buf, buflen, nframes)
  end subroutine

  subroutine db_unpack_batch_item(buf, buflen, idx, xyz, cap, natoms, status)
    integer(c_int8_t), intent(in) :: buf(*)
    integer(c_size_t), intent(in) :: buflen
    integer(c_int32_t), intent(in) :: idx, cap
    real(c_double), intent(out) :: xyz(*)
    integer(c_int32_t), intent(out) :: natoms
    integer(c_int), intent(out) :: status
    status = rkrdb_unpack_batch_item(buf, buflen, idx, xyz, cap, natoms)
  end subroutine

  subroutine db_unpack_positions(buf, buflen, xyz, cap, natoms, status)
    integer(c_int8_t), intent(in) :: buf(*)
    integer(c_size_t), intent(in) :: buflen
    real(c_double), intent(out) :: xyz(*)
    integer(c_int32_t), intent(in) :: cap
    integer(c_int32_t), intent(out) :: natoms
    integer(c_int), intent(out) :: status
    status = rkrdb_unpack_positions(buf, buflen, xyz, cap, natoms)
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

  subroutine db_append_units(id, traj_id, path, units_json, n_frames, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    character(len=*), intent(in) :: path, units_json
    integer(c_int32_t), intent(out) :: n_frames
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cp(:), cu(:)
    cp = f_c_string(path)
    cu = f_c_string(units_json)
    status = rkrdb_append_trajectory_units(id, traj_id, cp, cu, n_frames)
  end subroutine

  subroutine db_set_units(id, traj_id, units_json, n_frames, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    character(len=*), intent(in) :: units_json
    integer(c_int32_t), intent(out) :: n_frames
    integer(c_int), intent(out) :: status
    character(kind=c_char), allocatable :: cu(:)
    cu = f_c_string(units_json)
    status = rkrdb_set_units(id, traj_id, cu, n_frames)
  end subroutine

  subroutine db_h5md_times(id, traj_id, times, cap, n, status)
    integer(c_size_t), intent(in) :: id, cap
    integer(c_int64_t), intent(in) :: traj_id
    real(c_double), intent(out) :: times(*)
    integer(c_int32_t), intent(out) :: n
    integer(c_int), intent(out) :: status
    status = rkrdb_h5md_times(id, traj_id, times, cap, n)
  end subroutine

  subroutine db_h5md_shape(id, traj_id, nframes, natoms, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(out) :: nframes, natoms
    integer(c_int), intent(out) :: status
    status = rkrdb_h5md_shape(id, traj_id, nframes, natoms)
  end subroutine

  subroutine db_h5md_positions(id, traj_id, xyz, cap, nframes, natoms, status)
    integer(c_size_t), intent(in) :: id, cap
    integer(c_int64_t), intent(in) :: traj_id
    real(c_double), intent(out) :: xyz(*)
    integer(c_int32_t), intent(out) :: nframes, natoms
    integer(c_int), intent(out) :: status
    status = rkrdb_h5md_positions(id, traj_id, xyz, cap, nframes, natoms)
  end subroutine

  subroutine db_frame_units(id, traj_id, frame_idx, buf, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    character(len=*), intent(out) :: buf
    integer(c_int), intent(out) :: status
    character(kind=c_char) :: tmp(len(buf)+1)
    integer :: i, n
    tmp = c_null_char
    status = rkrdb_frame_units(id, traj_id, frame_idx, tmp, int(size(tmp), c_size_t))
    buf = ' '
    if (status /= rkrdb_ok) return
    n = 0
    do i = 1, size(tmp)
      if (tmp(i) == c_null_char) exit
      n = i
    end do
    if (n > 0) then
      do i = 1, min(n, len(buf))
        buf(i:i) = tmp(i)
      end do
    end if
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

  subroutine db_frame_formula(id, traj_id, frame_idx, formula, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    character(len=*), intent(out) :: formula
    integer(c_int), intent(out) :: status
    character(kind=c_char) :: buf(512)
    integer :: i, n
    buf = c_null_char
    status = rkrdb_frame_formula(id, traj_id, frame_idx, buf, int(512, c_size_t))
    formula = ""
    if (status /= rkrdb_ok) return
    n = 0
    do i = 1, 512
      if (buf(i) == c_null_char) exit
      n = n + 1
    end do
    if (n > 0) formula = transfer(buf(1:n), formula(1:n))
  end subroutine


  subroutine db_cook_frame(id, traj_id, frame_idx, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    integer(c_int), intent(out) :: status
    status = rkrdb_cook_frame(id, traj_id, frame_idx)
  end subroutine

  subroutine db_delete_cooked(id, traj_id, frame_idx, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    integer(c_int), intent(out) :: status
    status = rkrdb_delete_cooked(id, traj_id, frame_idx)
  end subroutine

  subroutine db_has_valid_cooked(id, traj_id, frame_idx, valid, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    logical, intent(out) :: valid
    integer(c_int), intent(out) :: status
    integer(c_int) :: v
    v = rkrdb_has_valid_cooked(id, traj_id, frame_idx)
    if (v < 0) then
      status = v
      valid = .false.
    else
      status = rkrdb_ok
      valid = (v == 1)
    end if
  end subroutine

  function db_get_frame(id, traj_id, frame_idx) result(p)
    ! RKRConFrame*; free with free_rkr_frame from readcon-core.
    integer(c_size_t), intent(in) :: id
    integer(c_int64_t), intent(in) :: traj_id
    integer(c_int32_t), intent(in) :: frame_idx
    type(c_ptr) :: p
    p = rkrdb_get_frame(id, traj_id, frame_idx)
  end function

  subroutine db_recook_all(id, status)
    integer(c_size_t), intent(in) :: id
    integer(c_int), intent(out) :: status
    status = rkrdb_recook_all(id)
  end subroutine

end module readcon_db
