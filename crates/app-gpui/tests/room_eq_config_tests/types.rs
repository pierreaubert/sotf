pub(super) type FilterTriple = (f64, f64, f64);

pub(super) type ChannelFilters<'a> = (&'a str, Vec<FilterTriple>);
