#!/usr/bin/env python3
"""Reject new SQLite imports until their minimum iOS version is checked."""
import subprocess
import sys

# Verified against Apple's SDK declarations. The newest is error_offset,
# available in iOS 16. changes64 needs 15.4 and prepare_v3 needs 12.
IOS_17_SYMBOLS = set('''
sqlite3_bind_parameter_count sqlite3_bind_text64 sqlite3_changes64 sqlite3_close
sqlite3_column_blob sqlite3_column_bytes sqlite3_column_count sqlite3_column_double
sqlite3_column_int64 sqlite3_column_name sqlite3_column_text sqlite3_column_type
sqlite3_errcode sqlite3_errmsg sqlite3_error_offset sqlite3_errstr sqlite3_finalize
sqlite3_get_autocommit sqlite3_prepare_v3 sqlite3_reset sqlite3_step
'''.split())

output = subprocess.check_output(['xcrun', 'nm', '-u', sys.argv[1]], text=True)
imports = {line.split()[-1].removeprefix('_') for line in output.splitlines() if '_sqlite3_' in line}
unknown = imports - IOS_17_SYMBOLS
if unknown:
    raise SystemExit('Check minimum iOS versions before allowing: ' + ', '.join(sorted(unknown)))
print(f'SQLite compatibility passed. {len(imports)} imports fit iOS 17.')
