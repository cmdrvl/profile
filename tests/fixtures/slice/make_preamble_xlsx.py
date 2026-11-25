"""Generates preamble_canary.xlsx (bd-ds5): a SYNTHETIC workbook shaped like a messy
multi-row-preamble report, with planted canary strings so tests can prove they never reach
stdout/stderr without --explicit.

Layout (sheet "Report"):
  row 1   merged A1:D1  banner paragraph          (canary BANNER)
  row 2   A2            methodology note          (canary NOTE)
  row 3   blank
  row 4   header: Loan ID | Balance | Status | Maturity
  rows 5-7 data rows with a canary in each of the first data rows (canary DATA5, DATA6)

`profile slice --skip-rows 0` mis-slices the banner into the header; `--skip-rows 4` makes the
first data row the header. Run: python3 make_preamble_xlsx.py preamble_canary.xlsx
"""
import sys

from openpyxl import Workbook

wb = Workbook()
ws = wb.active
ws.title = "Report"
ws["A1"] = "CANARY-BANNER-7f3a1c Quarterly methodology: figures are unaudited"
ws.merge_cells("A1:D1")
ws["A2"] = "CANARY-NOTE-2b9c4e prepared for internal use"
ws.append([])
ws.append(["Loan ID", "Balance", "Status", "Maturity"])
ws.append(["CANARY-DATA5-5d1e8a", 1000.5, "Current", "2027-01-01"])
ws.append(["CANARY-DATA6-9c0b3f", 2000, "Watch", "2027-06-01"])
ws.append(["L-0003", 3000.25, "Current", "2028-01-01"])
wb.create_sheet("Other")
wb.save(sys.argv[1])
