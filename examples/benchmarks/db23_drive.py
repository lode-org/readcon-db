"""Fair traj-100k driver.

Scratch and wheels are built under ``build/`` by ``db24_drive.py``.
That driver writes ``bench/db24``. This module no longer writes ``/tmp``.
"""

from db24_drive import main

if __name__ == "__main__":
    main()
