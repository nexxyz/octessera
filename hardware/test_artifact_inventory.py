from __future__ import annotations

import sys
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ENCLOSURE_ROOT = ROOT / "enclosure"
PCB_GERBER_ROOT = ROOT / "pcb" / "gerber"

ENCLOSURE_STEMS = (
    "case_bottom_plate_cadquery",
    "case_top_two_level_cadquery_orange-pi-zero-2w",
    "case_top_two_level_cadquery_raspberry-pi-zero-2w",
    "encoder_cap_aux1_ribbed_dot",
    "encoder_cap_aux2_ribbed_dots",
    "encoder_cap_aux3_ribbed_dots",
    "encoder_cap_main_knurled_dots",
    "friction_insert_expanding_plug_flat_sides",
    "friction_insert_spreading_pin_headed",
    "mx_keycap_back",
    "mx_keycap_fn_layer",
    "mx_keycap_play",
    "mx_keycap_shift",
    "protective_case_deep_tub_debossed_logo",
    "protective_case_shallow_lid_debossed_branding",
    "standoff_pillar_10mm",
    "standoff_pillar_9_5mm",
    "standoff_top_pin_thin_base",
)
STEP_FILES = {f"{stem}.step" for stem in ENCLOSURE_STEMS}
STL_FILES = {f"{stem}.stl" for stem in ENCLOSURE_STEMS}
SINGLE_MATERIAL_3MF_FILES = {f"{stem}.3mf" for stem in ENCLOSURE_STEMS}
MULTICOLOR_3MF_FILES = {
    "case_bottom_plate_cadquery.3mf",
    "case_top_two_level_orange-pi-zero-2w_multicolor.3mf",
    "case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf",
    "encoder_cap_aux1_ribbed_dot_multicolor_flush.3mf",
    "encoder_cap_aux2_ribbed_dots_multicolor_flush.3mf",
    "encoder_cap_aux3_ribbed_dots_multicolor_flush.3mf",
    "encoder_cap_main_knurled_dots_multicolor_flush.3mf",
    "friction_insert_expanding_plug_flat_sides.3mf",
    "friction_insert_spreading_pin_headed.3mf",
    "mx_keycap_back_multicolor_flush.3mf",
    "mx_keycap_fn_layer_multicolor_flush.3mf",
    "mx_keycap_play_multicolor_flush.3mf",
    "mx_keycap_shift_multicolor_flush.3mf",
    "protective_case_deep_tub_multicolor_logo.3mf",
    "protective_case_shallow_lid_multicolor_branding.3mf",
    "standoff_pillar_10mm.3mf",
    "standoff_pillar_9_5mm.3mf",
    "standoff_top_pin_thin_base.3mf",
}
SUPPORT_3MF_FILES = {
    "case_bottom_plate_cadquery.3mf",
    "friction_insert_expanding_plug_flat_sides.3mf",
    "friction_insert_spreading_pin_headed.3mf",
    "standoff_pillar_10mm.3mf",
    "standoff_pillar_9_5mm.3mf",
    "standoff_top_pin_thin_base.3mf",
}
GERBER_FILES = {
    "octessera-B_Cu.gbr",
    "octessera-B_Mask.gbr",
    "octessera-B_Paste.gbr",
    "octessera-B_Silkscreen.gbr",
    "octessera-Edge_Cuts.gbr",
    "octessera-F_Cu.gbr",
    "octessera-F_Mask.gbr",
    "octessera-F_Paste.gbr",
    "octessera-F_Silkscreen.gbr",
    "octessera-job.gbrjob",
    "octessera-NPTH.drl",
    "octessera-PTH.drl",
}
ZIP_REQUIRED_MEMBERS = {"[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"}


def check_directory(path: Path, expected: set[str], failures: list[str]) -> None:
    if not path.is_dir():
        failures.append(f"missing artifact directory: {path}")
        return

    entries = {entry.name: entry for entry in path.iterdir()}
    actual = set(entries)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing:
        failures.append(f"{path}: missing files: {', '.join(missing)}")
    if unexpected:
        failures.append(f"{path}: unexpected entries: {', '.join(unexpected)}")
    checkfit_names = sorted(name for name in actual if "checkfit" in name.casefold())
    if checkfit_names:
        failures.append(f"{path}: filename contains checkfit: {', '.join(checkfit_names)}")

    for name in sorted(expected):
        entry = entries.get(name)
        if entry is None:
            continue
        if not entry.is_file():
            failures.append(f"{entry}: expected a regular file")
        elif entry.stat().st_size == 0:
            failures.append(f"{entry}: file is empty")


def check_3mf(path: Path, failures: list[str]) -> None:
    if not path.is_file():
        return
    try:
        with zipfile.ZipFile(path) as archive:
            bad_member = archive.testzip()
            if bad_member is not None:
                failures.append(f"{path}: unreadable ZIP member: {bad_member}")
            members = set(archive.namelist())
            missing = sorted(ZIP_REQUIRED_MEMBERS - members)
            if missing:
                failures.append(f"{path}: missing ZIP members: {', '.join(missing)}")
            for member in archive.infolist():
                if b"checkfit" in archive.read(member).lower():
                    failures.append(f"{path}: member content contains checkfit: {member.filename}")
    except Exception as error:
        failures.append(f"{path}: unreadable ZIP package: {error}")


def check_gerber_zip(failures: list[str]) -> None:
    path = PCB_GERBER_ROOT / "gerber.zip"
    if not path.is_file():
        return
    try:
        with zipfile.ZipFile(path) as archive:
            bad_member = archive.testzip()
            if bad_member is not None:
                failures.append(f"{path}: unreadable ZIP member: {bad_member}")
            members = [info.filename for info in archive.infolist()]
            if set(members) != GERBER_FILES or len(members) != len(GERBER_FILES):
                failures.append(
                    f"{path}: expected exactly {len(GERBER_FILES)} fabrication members, "
                    f"found {', '.join(sorted(members))}"
                )
    except Exception as error:
        failures.append(f"{path}: unreadable ZIP package: {error}")


def main() -> int:
    failures: list[str] = []
    enclosure_directories = (
        (ENCLOSURE_ROOT / "step", STEP_FILES),
        (ENCLOSURE_ROOT / "stl", STL_FILES),
        (ENCLOSURE_ROOT / "3mf-single-material", SINGLE_MATERIAL_3MF_FILES),
        (ENCLOSURE_ROOT / "3mf-multicolor", MULTICOLOR_3MF_FILES),
    )
    for path, expected in enclosure_directories:
        check_directory(path, expected, failures)

    for directory in (ENCLOSURE_ROOT / "3mf-single-material", ENCLOSURE_ROOT / "3mf-multicolor"):
        for name in sorted(SUPPORT_3MF_FILES):
            if not (directory / name).is_file():
                failures.append(f"{directory}: missing one-material support 3MF: {name}")

    for directory, expected in enclosure_directories[2:]:
        for name in sorted(expected):
            check_3mf(directory / name, failures)

    check_directory(PCB_GERBER_ROOT, GERBER_FILES | {"gerber.zip"}, failures)
    check_gerber_zip(failures)

    if failures:
        print("Hardware artifact inventory FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("Hardware artifact inventory passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
