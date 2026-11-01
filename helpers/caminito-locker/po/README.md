# caminito-locker's translations

The domain has the strings the Locker and `mate-screensaver-command` use, copied from the `.po`
files of mate-screensaver `60b2db3` (GPL-2.0-or-later) with their translators' notices (ADR 0056).
To copy them again, with `$ms` a checkout of mate-screensaver and from this directory:

```sh
(cd ../../.. && xgettext --from-code=UTF-8 -k_ -kN_ --add-comments --package-name=caminito-locker \
  --files-from=helpers/caminito-locker/po/POTFILES -o helpers/caminito-locker/po/caminito-locker.pot)
cp "$ms/po/LINGUAS" .
for lang in $(cat LINGUAS); do
  msgmerge --quiet --no-fuzzy-matching --no-location "$ms/po/$lang.po" caminito-locker.pot |
    msgattrib --no-obsolete -o "$lang.po"
done
```

A string that isn't in mate-screensaver's catalogues shows in English until it's translated here.
