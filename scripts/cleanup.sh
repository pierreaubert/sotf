#!/bin/bash

cargo clean
rm -fr ./dist
rm -fr ./data_generated/*
rm -fr ./sotf-audio-player/app-gpui/components/
rm -fr mutants.out
find . -name '*~' -exec rm {} \; -print
find . -name '*.(log|out)' -exec rm {} \; -print
find . -name 'Cargo.lock' -exec rm {} \; -print
