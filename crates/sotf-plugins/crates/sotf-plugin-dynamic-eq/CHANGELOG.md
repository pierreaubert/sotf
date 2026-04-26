# 0.5.5

- Band frequency / q live updates now rebuild sidechain and EQ filters immediately.
- Reset() now clears current_gain_db before rebuilding EQ filters, so reset returns the band to neutral gain.
- Process_in_place now checks frame/channel overflow and exact buffer length, returning Err instead of panicking.
