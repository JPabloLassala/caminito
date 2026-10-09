#!/usr/bin/env bash
# The dbusmenu traffic and claims in the session's bus log from line $2 on, one line each:
# events.sh <work> <line>
tail -n +"$2" "$1/out/bus.log" | awk '
  /^(signal|method call)/ { if (line) print line; match($0, /time=[0-9.]+/); t=substr($0, RSTART+5, RLENGTH-5);
    match($0, /member=[A-Za-z]+/); m=substr($0, RSTART+7, RLENGTH-7); line=sprintf("%.3f %s", t % 1000, m); n=0; next }
  /^(method return|error)/ { if (line) print line; line="" ; next }
  line && n < 2 && /^ +(int32|uint32|string) / { gsub(/^ +/, ""); line=line " " $0; n++ }
  END { if (line) print line }'
