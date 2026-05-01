#!/bin/sh
rsync -avrz dist/* --delete spin@vps-c2ea73ea.vps.ovh.net:/var/www/html/spinorama-sotf
