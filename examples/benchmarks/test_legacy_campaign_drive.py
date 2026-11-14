"""The historical fair campaigns keep the logged text sizes."""

from __future__ import annotations

import legacy_campaign_drive as drive


def test_every_campaign_has_a_mode_and_its_logged_sizes() -> None:
    assert set(drive.CAMPAIGNS) == {f"db{number}" for number in range(1, 15)}
    for spec in drive.CAMPAIGNS.values():
        assert spec["mode"] in {"all", "db8", "db9", "db10", "db11", "db12", "db14"}
        assert spec["bytes"]
        assert all(size > 0 for size in spec["bytes"].values())
    assert drive.CAMPAIGNS["db1"]["bytes"] == drive.CAMPAIGNS["db7"]["bytes"]
    assert drive.CAMPAIGNS["db13"]["mode"] == "db10"
    assert drive.CAMPAIGNS["db13"]["bytes"] == {"frames-100": 73625}
    assert drive.CAMPAIGNS["db8"]["bytes"]["atoms-1000000"] == 54086580
