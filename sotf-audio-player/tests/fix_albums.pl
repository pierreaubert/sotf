#!/usr/bin/env perl
use strict;
use warnings;

my $file = $ARGV[0];
open my $fh, '<', $file or die "Cannot open $file: $!";
my @lines = <$fh>;
close $fh;

for my $i (0..$#lines) {
    if ($lines[$i] =~ /^\s+album_art_path:\s*/) {
        # Check if next line doesn't have play_count
        if ($i < $#lines && $lines[$i+1] !~ /play_count/) {
            # Insert play_count before the closing brace
            if ($lines[$i+1] =~ /^(\s+)\}/) {
                my $indent = $1;
                $lines[$i] =~ s/,?\s*$/,\n/;
                splice @lines, $i+1, 0, "${indent}play_count: 0,\n";
            }
        }
    }
}

open $fh, '>', $file or die "Cannot write $file: $!";
print $fh @lines;
close $fh;
