#!/bin/bash
# The whole campaign, one variant at a time (each holds heavy lock slot 2 while it runs).
# (First started with a longer list; trimmed after g-o2 because the lock was shared with other lanes. Dropped: g-cgu4, r-o2, g-abort, r-dbg, g-native.)
cd "$(dirname "$0")"
R=./run_variant.sh
$R release release
$R g-o1 g-o1
$R g-o2 g-o2
$R g-os g-os
$R g-oz g-oz
$R g-cgu1 g-cgu1
$R g-thin g-thin
$R g-fat g-fat
$R r-fat r-fat
$R r-abort r-abort
$R r-ovf r-ovf
$R r-v3 release "-C target-cpu=x86-64-v3"
$R r-native release "-C target-cpu=native"
echo ALLDONE >> /root/deploytmp/flags-data/done
