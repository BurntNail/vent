# Changelog

## 0.1.0 (2023-06-06)


### Features

* add ability for devs to reload partials ([a0adbbc](https://github.com/BurntNail/knot/commit/a0adbbc6299c6331005be26961c37b0eb65da268))
* add ability to rename website from `Knot`. ([b8ba2a2](https://github.com/BurntNail/knot/commit/b8ba2a2e177e19d1cbc79c1672aaec3617728db9))
* add authentication roles for different perm levels ([885328b](https://github.com/BurntNail/knot/commit/885328b298289c0f5ce0291f767ce03bac107f4a))
* add authentication w/ passwords + usernames ([4daccb7](https://github.com/BurntNail/knot/commit/4daccb7cb52029ee439bcf7fa9ba815c00e7cf29))
* add export to spreadsheet ([66a099e](https://github.com/BurntNail/knot/commit/66a099eb5e431eada37514f424a5353841a09c18))
* add tooltips for numbers over the numbers info box ([a2e31e5](https://github.com/BurntNail/knot/commit/a2e31e52eaf60b950843db44e8c6612df262bc14))
* can't see photos unless logged in ([8d60da4](https://github.com/BurntNail/knot/commit/8d60da40930f67b22b67a47abab2026dee430433))
* change names of forms ([#53](https://github.com/BurntNail/knot/issues/53)) ([7911b50](https://github.com/BurntNail/knot/commit/7911b5021c13ff029be4d22f36c8540213efc597))
* import events + instructions ([ec0304c](https://github.com/BurntNail/knot/commit/ec0304c504fd6e9c3108cc755e4094e67bfc4603))
* login by username ([4d809f4](https://github.com/BurntNail/knot/commit/4d809f4bd0fe42792e4ed535366841ce8af9516b))
* make all people users ([#55](https://github.com/BurntNail/knot/issues/55)) ([d17eccb](https://github.com/BurntNail/knot/commit/d17eccb8447460cb83bf679b73e8086f6cc5b28f))
* **sec:** add cloudflare turnstile to forms. ([#43](https://github.com/BurntNail/knot/issues/43)) ([e23b82c](https://github.com/BurntNail/knot/commit/e23b82c67491f57af27dd22c0e5ae9218392ef19))
* set password with magic link from email ([4d809f4](https://github.com/BurntNail/knot/commit/4d809f4bd0fe42792e4ed535366841ce8af9516b))
* show number of photos on an event in index ([8036f91](https://github.com/BurntNail/knot/commit/8036f91a74e8ff5cc4cf9df3d0345007ecda68d8))


### Bug Fixes

* adding people is now a checkbox not individual links ([8d8b4bb](https://github.com/BurntNail/knot/commit/8d8b4bb517c69f08c4b896fab1ec0cbdc41ac02e))
* **deps:** update rust crate chrono to 0.4.26 ([#48](https://github.com/BurntNail/knot/issues/48)) ([901a034](https://github.com/BurntNail/knot/commit/901a034a09aef1154760abad30ccb21f67fab339))
* **deps:** update rust crate liquid to 0.26.3 ([#63](https://github.com/BurntNail/knot/issues/63)) ([dad64c3](https://github.com/BurntNail/knot/commit/dad64c3e1141f8387c938e17dc22db2d8a438bba))
* **deps:** update rust crate once_cell to 1.18.0 ([#65](https://github.com/BurntNail/knot/issues/65)) ([5b61080](https://github.com/BurntNail/knot/commit/5b6108060e0bd10919b7fcc2e60c08c08240461e))
* **deps:** update rust crate rust_xlsxwriter to 0.40.0 ([#49](https://github.com/BurntNail/knot/issues/49)) ([e937d47](https://github.com/BurntNail/knot/commit/e937d4754e111330993ce58c6b852ec1e90b1a62))
* error page now looks similar to rest of UI and directs user to issues page ([4fe5405](https://github.com/BurntNail/knot/commit/4fe5405f6e2d5b6b64969d81400ca3d799580196))
* less liberal use of turnstile ([#46](https://github.com/BurntNail/knot/issues/46)) ([28a0261](https://github.com/BurntNail/knot/commit/28a0261805e3e9ce32a484ccc2d0b08b17a18906))
* make sure people are ordered correctly (form/surname) on spreadsheet export ([e195ca1](https://github.com/BurntNail/knot/commit/e195ca158486a5f28ec46991ec19767d06c3ce5e))
* no use of `&mut conn`, in favour of `pool.as_ref()` to avoid connections staying open when not being used. ([d09bfe5](https://github.com/BurntNail/knot/commit/d09bfe55dc2e9c960c8223b8603aa3e678b5c29e))
* password emailing from serverside ([81cc723](https://github.com/BurntNail/knot/commit/81cc723f3996ed6aae47d38b99ccd443e6a795c1))
* remove `is_prefects` as now outdated by permission roles ([#57](https://github.com/BurntNail/knot/issues/57)) ([95f9c9c](https://github.com/BurntNail/knot/commit/95f9c9c65d1b945226e2dcfeb76a8de64c59f917))
* tooltip anchors now take you to that section, not the base of the page ([87601c7](https://github.com/BurntNail/knot/commit/87601c7e0377f247d13ff779a17a4391fe0fa79b))
* turnstile fix ([c207c27](https://github.com/BurntNail/knot/commit/c207c27eb1ef3d7b0f7d21c9a3684d9c1847e9a7))
* when login fails, explain why, and fix name misspellings leading to 500 row not found ([b1b5f39](https://github.com/BurntNail/knot/commit/b1b5f39dcba08e41cfca29259c3e280874541645))


### Performance Improvements

* make sure not to re-serve the same contents of zip files to save hdd space ([06dcb92](https://github.com/BurntNail/knot/commit/06dcb9299626e69143d6f1e3fdf362bae8263631))


### Miscellaneous Chores

* change release version ([05a37fc](https://github.com/BurntNail/knot/commit/05a37fcbc142432f5d6649283510f75a6cb16b19))
