object CONVERT_UTILS
  name: "Conversion Utils"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property "%" (owner: HACKER, flags: "r") = "1|100";
  property abampere (owner: HACKER, flags: "r") = "10 amp";
  property abcoulomb (owner: HACKER, flags: "r") = "10 coul";
  property abfarad (owner: HACKER, flags: "r") = "10 farad";
  property abhenry (owner: HACKER, flags: "r") = "10 henry";
  property abmho (owner: HACKER, flags: "r") = "10 mho";
  property abohm (owner: HACKER, flags: "r") = "10 ohm";
  property abvolt (owner: HACKER, flags: "r") = "10 volt";
  property acre (owner: HACKER, flags: "r") = "43560 ft2";
  property amp (owner: HACKER, flags: "r") = "ampere";
  property ampere (owner: HACKER, flags: "r") = "coul/sec";
  property amu (owner: HACKER, flags: "r") = "chemamu";
  property angstrom (owner: HACKER, flags: "r") = "1e-8 meter";
  property apdram (owner: HACKER, flags: "r") = "60 grain";
  property apostilb (owner: HACKER, flags: "r") = "cd/pi m2";
  property apounce (owner: HACKER, flags: "r") = "480 grain";
  property appound (owner: HACKER, flags: "r") = "5760 grain";
  property arcdeg (owner: HACKER, flags: "r") = "1 degree";
  property arcmin (owner: HACKER, flags: "r") = "1|60 arcdeg";
  property arcsec (owner: HACKER, flags: "r") = "1|60 arcmin";
  property are (owner: HACKER, flags: "r") = "100 m2";
  property arpentcan (owner: HACKER, flags: "r") = "27.52 mi";
  property arpentlin (owner: HACKER, flags: "r") = "191.835 ft";
  property astronomicalunit (owner: #2, flags: "r") = "au";
  property atm (owner: HACKER, flags: "r") = "atmosphere";
  property atmosphere (owner: HACKER, flags: "r") = "1.01325 bar";
  property atomicmassunit (owner: HACKER, flags: "r") = "amu";
  property au (owner: HACKER, flags: "r") = "1.49599e11 m";
  property avdram (owner: HACKER, flags: "r") = "1|16 oz";
  property bag (owner: HACKER, flags: "r") = "3 brbushels";
  property bakersdozen (owner: HACKER, flags: "r") = "13";
  property bar (owner: HACKER, flags: "r") = "1e6 dyne/cm2";
  property barie (owner: HACKER, flags: "r") = "1e-1 nt/m2";
  property barleycorn (owner: HACKER, flags: "r") = "1|3 in";
  property barn (owner: HACKER, flags: "r") = "1e-24 cm2";
  property barrel (owner: HACKER, flags: "r") = "31.5 gal";
  property barye (owner: HACKER, flags: "r") = "1e-1 nt/m2";
  property basic_units (owner: HACKER, flags: "r") = {"m", "kg", "s", "coul", "candela", "radian", "bit", "erlang", "kelvin"};
  property basic_units_template (owner: HACKER, flags: "r") = {
    {"m", 0},
    {"kg", 0},
    {"s", 0},
    {"coul", 0},
    {"candela", 0},
    {"radian", 0},
    {"bit", 0},
    {"erlang", 0},
    {"kelvin", 0}
  };
  property baud (owner: HACKER, flags: "r") = "bit/sec";
  property bev (owner: HACKER, flags: "r") = "1e9 ev";
  property biot (owner: HACKER, flags: "r") = "10 amp";
  property block (owner: HACKER, flags: "r") = "512 byte";
  property blondel (owner: HACKER, flags: "r") = "cd/pi m2";
  property boardfeet (owner: HACKER, flags: "r") = "boardfoot";
  property boardfoot (owner: HACKER, flags: "r") = "144 in3";
  property bolt (owner: HACKER, flags: "r") = "120 feet";
  property bottommeasure (owner: HACKER, flags: "r") = "1|40 in";
  property brbarrel (owner: HACKER, flags: "r") = "4.5 brbushels";
  property brbucket (owner: HACKER, flags: "r") = "4 dry british gal";
  property brbushel (owner: HACKER, flags: "r") = "8 dry british gal";
  property brfirkin (owner: HACKER, flags: "r") = "1.125 brbushel";
  property british (owner: HACKER, flags: "r") = "277.4193|231";
  property brknot (owner: HACKER, flags: "r") = "6080 ft/hr";
  property brpeck (owner: HACKER, flags: "r") = "2 dry british gal";
  property btu (owner: HACKER, flags: "r") = "1054.35 joule";
  property bu (owner: HACKER, flags: "r") = "bushel";
  property bushel (owner: HACKER, flags: "r") = "8 dry gal";
  property butt (owner: HACKER, flags: "r") = "126 gal";
  property byte (owner: HACKER, flags: "r") = "8 bit";
  property c (owner: HACKER, flags: "r") = "2.99792458e8 m/sec";
  property cable (owner: HACKER, flags: "r") = "720 ft";
  property cal (owner: HACKER, flags: "r") = "4.1868 joule";
  property caliber (owner: HACKER, flags: "r") = "0.01 in";
  property calorie (owner: HACKER, flags: "r") = "cal";
  property candle (owner: HACKER, flags: "r") = "cd";
  property candlepower (owner: HACKER, flags: "r") = "12.566370 lumen";
  property carat (owner: HACKER, flags: "r") = "205.3 mg";
  property carcel (owner: HACKER, flags: "r") = "9.61 cd";
  property cc (owner: HACKER, flags: "r") = "cm3";
  property ccs (owner: HACKER, flags: "r") = "1|36 erlang";
  property cd (owner: HACKER, flags: "r") = "candela";
  property cental (owner: HACKER, flags: "r") = "100 lb";
  property centare (owner: HACKER, flags: "r") = "0.01 are";
  property centesimalminute (owner: HACKER, flags: "r") = "1e-2 grade";
  property centesimalsecond (owner: HACKER, flags: "r") = "1e-4 grade";
  property century (owner: HACKER, flags: "r") = "100 year";
  property cfs (owner: HACKER, flags: "r") = "ft3/sec";
  property cg (owner: HACKER, flags: "r") = "centigram";
  property chain (owner: HACKER, flags: "r") = "gunterchain";
  property chemamu (owner: HACKER, flags: "r") = "1.66024e-24 g";
  property chemdalton (owner: HACKER, flags: "r") = "chemamu";
  property circle (owner: HACKER, flags: "r") = "2 pi radian";
  property circularinch (owner: HACKER, flags: "r") = "1|4 pi in2";
  property circularmil (owner: HACKER, flags: "r") = "1e-6|4 pi in2";
  property clusec (owner: HACKER, flags: "r") = "1e-8 mm hg m3/s";
  property cm (owner: HACKER, flags: "r") = "centimeter";
  property coomb (owner: HACKER, flags: "r") = "4 bu";
  property cord (owner: HACKER, flags: "r") = "128 ft3";
  property cordfeet (owner: HACKER, flags: "r") = "cordfoot";
  property cordfoot (owner: HACKER, flags: "r") = "0.125 cord";
  property coul (owner: HACKER, flags: "r") = "coulomb";
  property cps (owner: HACKER, flags: "r") = "hertz";
  property crith (owner: HACKER, flags: "r") = "9.06e-2 gram";
  property cubichectare (owner: HACKER, flags: "r") = "1000000 m3";
  property cubit (owner: HACKER, flags: "r") = "18 in";
  property cup (owner: HACKER, flags: "r") = "1|2 pt";
  property curie (owner: HACKER, flags: "r") = "3.7e10/sec";
  property dalton (owner: HACKER, flags: "r") = "chemamu";
  property day (owner: HACKER, flags: "r") = "24 hr";
  property decade (owner: HACKER, flags: "r") = "10 year";
  property degree (owner: HACKER, flags: "r") = "1|180 pi radian";
  property dioptre (owner: HACKER, flags: "r") = "1/m";
  property displacementton (owner: HACKER, flags: "r") = "35 ft3";
  property dopplezentner (owner: HACKER, flags: "r") = "100 kg";
  property dozen (owner: HACKER, flags: "r") = "12";
  property dr (owner: HACKER, flags: "r") = "dram";
  property drachm (owner: HACKER, flags: "r") = "3.551531 ml";
  property dram (owner: HACKER, flags: "r") = "avdram";
  property dry (owner: HACKER, flags: "r") = "268.8025 in3/gallon";
  property dryquartern (owner: HACKER, flags: "r") = "2.272980 l";
  property dyne (owner: HACKER, flags: "r") = "erg/cm";
  property e (owner: HACKER, flags: "r") = "1.6020e-19 coul";
  property electronvolt (owner: HACKER, flags: "r") = "e volt";
  property ell (owner: HACKER, flags: "r") = "45 in";
  property energy (owner: HACKER, flags: "r") = "c2";
  property engcandle (owner: HACKER, flags: "r") = "1.04 cd";
  property engineerschain (owner: HACKER, flags: "r") = "100 ft";
  property engineerslink (owner: HACKER, flags: "r") = "0.01 engineerschain";
  property equivalentfootcandle (owner: HACKER, flags: "r") = "lumen/pi ft2";
  property equivalentlux (owner: HACKER, flags: "r") = "lumen/pi m2";
  property equivalentphot (owner: HACKER, flags: "r") = "cd/pi cm2";
  property erg (owner: HACKER, flags: "r") = "1e-7 joule";
  property ev (owner: HACKER, flags: "r") = "e volt";
  property farad (owner: HACKER, flags: "r") = "coul/volt";
  property faraday (owner: HACKER, flags: "r") = "9.648456e4coul";
  property fathom (owner: HACKER, flags: "r") = "6 ft";
  property feet (owner: HACKER, flags: "r") = "foot";
  property fermi (owner: HACKER, flags: "r") = "1e-13 cm";
  property fifth (owner: HACKER, flags: "r") = "1|5 gal";
  property finger (owner: HACKER, flags: "r") = "7|8 in";
  property firkin (owner: HACKER, flags: "r") = "72 pint";
  property fldr (owner: HACKER, flags: "r") = "1|32 gill";
  property floz (owner: HACKER, flags: "r") = "1|4 gill";
  property fluidounce (owner: #2, flags: "r") = "floz";
  property foot (owner: HACKER, flags: "r") = "12 in";
  property footcandle (owner: HACKER, flags: "r") = "lumen/ft2";
  property footlambert (owner: HACKER, flags: "r") = "cd/pi ft2";
  property force (owner: HACKER, flags: "r") = "g";
  property fortnight (owner: HACKER, flags: "r") = "14 day";
  property franklin (owner: HACKER, flags: "r") = "3.33564e-10 coul";
  property frigorie (owner: HACKER, flags: "r") = "kilocal";
  property ft (owner: HACKER, flags: "r") = "foot";
  property furlong (owner: HACKER, flags: "r") = "660 ft";
  property g (owner: HACKER, flags: "r") = "9.80665 m/sec2";
  property gal (owner: HACKER, flags: "r") = "gallon";
  property galileo (owner: HACKER, flags: "r") = "1e-2 m/sec2";
  property gallon (owner: HACKER, flags: "r") = "231 in3";
  property gamma (owner: HACKER, flags: "r") = "1e-6 g";
  property gauss (owner: HACKER, flags: "r") = "maxwell/cm2";
  property gb (owner: HACKER, flags: "r") = "1024 mb";
  property geographicalmile (owner: HACKER, flags: "r") = "nmile";
  property germancandle (owner: HACKER, flags: "r") = "1.05 cd";
  property gilbert (owner: HACKER, flags: "r") = "0.7957747154 amp";
  property gill (owner: HACKER, flags: "r") = "1|4 pt";
  property gm (owner: HACKER, flags: "r") = "gram";
  property gr (owner: HACKER, flags: "r") = "grain";
  property grad (owner: HACKER, flags: "r") = "1|400 circle";
  property grade (owner: HACKER, flags: "r") = "1|400 circle";
  property grain (owner: HACKER, flags: "r") = "1|7000 lb";
  property gram (owner: HACKER, flags: "r") = "1|1000 kg";
  property gramcalorie (owner: HACKER, flags: "r") = "cal";
  property gross (owner: HACKER, flags: "r") = "144";
  property gunterchain (owner: HACKER, flags: "r") = "66 ft";
  property gunterlink (owner: HACKER, flags: "r") = "0.01 gunterchain";
  property hand (owner: HACKER, flags: "r") = "4 in";
  property hd (owner: HACKER, flags: "r") = "hogshead";
  property hectare (owner: HACKER, flags: "r") = "100 are";
  property hefnercandle (owner: HACKER, flags: "r") = "hefnerunit";
  property hefnerunit (owner: HACKER, flags: "r") = ".92 cd";
  property henry (owner: HACKER, flags: "r") = "sec2/farad";
  property hertz (owner: HACKER, flags: "r") = "1/sec";
  property hg (owner: HACKER, flags: "r") = "mercury";
  property hogshead (owner: HACKER, flags: "r") = "63 gal";
  property homestead (owner: HACKER, flags: "r") = "1|4 mi2";
  property horsepower (owner: HACKER, flags: "r") = "550 ft lb g/sec";
  property hour (owner: HACKER, flags: "r") = "60 min";
  property hp (owner: HACKER, flags: "r") = "horsepower";
  property hr (owner: HACKER, flags: "r") = "hour";
  property hyl (owner: HACKER, flags: "r") = "gram force sec2/m";
  property hz (owner: HACKER, flags: "r") = "hertz";
  property imaginarycubicfoot (owner: HACKER, flags: "r") = "1.4 ft3";
  property imperial (owner: HACKER, flags: "r") = "1.200949";
  property in (owner: HACKER, flags: "r") = "inch";
  property inch (owner: HACKER, flags: "r") = "2.54 cm";
  property inches (owner: HACKER, flags: "r") = "inch";
  property jeroboam (owner: HACKER, flags: "r") = "4|5 gal";
  property joule (owner: HACKER, flags: "r") = "nt m";
  property k (owner: HACKER, flags: "r") = "1.38047e-16 erg/kelvin";
  property karat (owner: HACKER, flags: "r") = "1|24";
  property kb (owner: HACKER, flags: "r") = "1024 byte";
  property kcal (owner: HACKER, flags: "r") = "kilocal";
  property kcalorie (owner: HACKER, flags: "r") = "kilocal";
  property kev (owner: HACKER, flags: "r") = "1e3 ev";
  property khz (owner: HACKER, flags: "r") = "kilohz";
  property kilderkin (owner: HACKER, flags: "r") = "18 imperial gal";
  property km (owner: HACKER, flags: "r") = "kilometer";
  property knot (owner: HACKER, flags: "r") = "nmile/hr";
  property l (owner: HACKER, flags: "r") = "liter";
  property lambert (owner: HACKER, flags: "r") = "cd/pi cm2";
  property langley (owner: HACKER, flags: "r") = "cal/cm cm";
  property last (owner: HACKER, flags: "r") = "80 bu";
  property lb (owner: HACKER, flags: "r") = "0.45359237 kg";
  property lbf (owner: HACKER, flags: "r") = "lb g";
  property league (owner: HACKER, flags: "r") = "3 mi";
  property lightyear (owner: HACKER, flags: "r") = "c yr";
  property line (owner: HACKER, flags: "r") = "1|12 in";
  property link (owner: HACKER, flags: "r") = "66|100 ft";
  property liqquarten (owner: HACKER, flags: "r") = "0.1420613 l";
  property liter (owner: HACKER, flags: "r") = "1000 cc";
  property long (owner: HACKER, flags: "r") = "4 word";
  property longhundredweight (owner: HACKER, flags: "r") = "112 lb";
  property longquarter (owner: HACKER, flags: "r") = "28 lb";
  property longton (owner: HACKER, flags: "r") = "2240 lb";
  property lumen (owner: HACKER, flags: "r") = "cd sr";
  property lusec (owner: HACKER, flags: "r") = "1e-6 mm hg m3/s";
  property lux (owner: HACKER, flags: "r") = "lumen/m2";
  property mach (owner: HACKER, flags: "r") = "331.45 m/sec";
  property magnum (owner: HACKER, flags: "r") = "2 qt";
  property marineleague (owner: HACKER, flags: "r") = "3nmile";
  property maxwell (owner: HACKER, flags: "r") = "1e-8 weber";
  property mb (owner: HACKER, flags: "r") = "1024 kb";
  property mercury (owner: HACKER, flags: "r") = "1.3157895 atm/m";
  property meter (owner: HACKER, flags: "r") = "m";
  property metriccarat (owner: HACKER, flags: "r") = "200 mg";
  property metricton (owner: HACKER, flags: "r") = "1000 kg";
  property mev (owner: HACKER, flags: "r") = "1e6 ev";
  property mg (owner: HACKER, flags: "r") = "milligram";
  property mgd (owner: HACKER, flags: "r") = "megagal/day";
  property mh (owner: HACKER, flags: "r") = "millihenry";
  property mho (owner: HACKER, flags: "r") = "1/ohm";
  property mhz (owner: HACKER, flags: "r") = "megahz";
  property mi (owner: HACKER, flags: "r") = "mile";
  property micron (owner: HACKER, flags: "r") = "1e-6 meter";
  property mil (owner: HACKER, flags: "r") = "0.001 in";
  property mile (owner: HACKER, flags: "r") = "5280 feet";
  property millenium (owner: HACKER, flags: "r") = "1000 year";
  property min (owner: HACKER, flags: "r") = "minute";
  property minersinch (owner: HACKER, flags: "r") = "1.5 ft3/min";
  property minim (owner: HACKER, flags: "r") = "1|480 floz";
  property minute (owner: HACKER, flags: "r") = "60 sec";
  property ml (owner: HACKER, flags: "r") = "milliliter";
  property mm (owner: HACKER, flags: "r") = "millimeter";
  property mo (owner: HACKER, flags: "r") = "month";
  property mole (owner: HACKER, flags: "r") = "6.022045e23";
  property month (owner: HACKER, flags: "r") = "1|12 year";
  property mpg (owner: HACKER, flags: "r") = "mile/gal";
  property mph (owner: HACKER, flags: "r") = "mile/hr";
  property ms (owner: HACKER, flags: "r") = "millisec";
  property myriagram (owner: HACKER, flags: "r") = "10 kg";
  property nail (owner: HACKER, flags: "r") = "1|16 yd";
  property nautleague (owner: HACKER, flags: "r") = "3 nmile";
  property nautmile (owner: HACKER, flags: "r") = "nmile";
  property newton (owner: HACKER, flags: "r") = "kg m/sec2";
  property nit (owner: HACKER, flags: "r") = "cd/m2";
  property nm (owner: HACKER, flags: "r") = "nanometer";
  property nmile (owner: HACKER, flags: "r") = "1852 m";
  property noggin (owner: HACKER, flags: "r") = "1 imperial gill";
  property nox (owner: HACKER, flags: "r") = "1e-3 lux";
  property ns (owner: HACKER, flags: "r") = "nanosec";
  property nt (owner: HACKER, flags: "r") = "newton";
  property oe (owner: HACKER, flags: "r") = "oersted";
  property oersted (owner: HACKER, flags: "r") = "1 gilbert / cm";
  property ohm (owner: HACKER, flags: "r") = "volt/amp";
  property ounce (owner: HACKER, flags: "r") = "1|16 lb";
  property oz (owner: HACKER, flags: "r") = "ounce";
  property pace (owner: HACKER, flags: "r") = "30 inch";
  property palm (owner: HACKER, flags: "r") = "3 in";
  property parsec (owner: HACKER, flags: "r") = "au radian/arcsec";
  property pascal (owner: HACKER, flags: "r") = "nt/m2";
  property pc (owner: HACKER, flags: "r") = "parsec";
  property pdl (owner: HACKER, flags: "r") = "poundal";
  property peck (owner: HACKER, flags: "r") = "2 dry gallon";
  property pennyweight (owner: HACKER, flags: "r") = "24 grain";
  property percent (owner: HACKER, flags: "r") = "%";
  property perch (owner: HACKER, flags: "r") = "24.75 ft3";
  property petrbarrel (owner: HACKER, flags: "r") = "42 gal";
  property pf (owner: HACKER, flags: "r") = "picofarad";
  property phot (owner: HACKER, flags: "r") = "lumen/cm2";
  property physamu (owner: HACKER, flags: "r") = "1.65979e-24 g";
  property physdalton (owner: HACKER, flags: "r") = "physamu";
  property pi (owner: HACKER, flags: "r") = "3.14159265358979323846264338327950288";
  property pica (owner: HACKER, flags: "r") = "0.166044 inch";
  property pieze (owner: HACKER, flags: "r") = "1e3 nt/mt2";
  property pint (owner: HACKER, flags: "r") = "1|2 qt";
  property pipe (owner: HACKER, flags: "r") = "4 barrel";
  property pk (owner: HACKER, flags: "r") = "peck";
  property point (owner: HACKER, flags: "r") = "1|72.27 in";
  property poise (owner: HACKER, flags: "r") = "gram/cm sec";
  property pole (owner: HACKER, flags: "r") = "rd";
  property pound (owner: HACKER, flags: "r") = "lb";
  property poundal (owner: HACKER, flags: "r") = "ft lb/sec2";
  property proof (owner: HACKER, flags: "r") = "1|200";
  property ps (owner: HACKER, flags: "r") = "picosec";
  property psi (owner: HACKER, flags: "r") = "lb g/in2";
  property pt (owner: HACKER, flags: "r") = "pint";
  property puncheon (owner: HACKER, flags: "r") = "84 gal";
  property qt (owner: HACKER, flags: "r") = "quart";
  property quadrant (owner: HACKER, flags: "r") = "5400 minute";
  property quart (owner: HACKER, flags: "r") = "1|4 gal";
  property quarter (owner: HACKER, flags: "r") = "9 in";
  property quartersection (owner: HACKER, flags: "r") = "1|4 mi2";
  property quintal (owner: HACKER, flags: "r") = "100 kg";
  property quire (owner: HACKER, flags: "r") = "25";
  property ra (owner: HACKER, flags: "r") = "100 erg/gram";
  property ramdenchain (owner: HACKER, flags: "r") = "100 ft";
  property ramdenlink (owner: HACKER, flags: "r") = "0.01 ramdenchain";
  property rankine (owner: HACKER, flags: "r") = "1.8 kelvin";
  property rd (owner: HACKER, flags: "r") = "rod";
  property ream (owner: HACKER, flags: "r") = "500";
  property refrigeration (owner: HACKER, flags: "r") = "12000 but/ton hr";
  property registerton (owner: HACKER, flags: "r") = "100 ft3";
  property rehoboam (owner: HACKER, flags: "r") = "156 floz";
  property revolution (owner: HACKER, flags: "r") = "360 degrees";
  property reyn (owner: HACKER, flags: "r") = "6.89476e-6 centipoise";
  property rhe (owner: HACKER, flags: "r") = "1/poise";
  property rod (owner: HACKER, flags: "r") = "16.5 ft";
  property rontgen (owner: HACKER, flags: "r") = "2.58e-4 curie/kg";
  property rood (owner: HACKER, flags: "r") = "0.25 acre";
  property rope (owner: HACKER, flags: "r") = "20 ft";
  property rpm (owner: HACKER, flags: "r") = "revolution/minute";
  property rutherford (owner: HACKER, flags: "r") = "1e6/sec";
  property rydberg (owner: HACKER, flags: "r") = "1.36054e1 ev";
  property sabin (owner: HACKER, flags: "r") = "1 ft2";
  property scruple (owner: HACKER, flags: "r") = "20 grain";
  property seam (owner: HACKER, flags: "r") = "8 brbushels";
  property sec (owner: HACKER, flags: "r") = "second";
  property second (owner: HACKER, flags: "r") = "s";
  property section (owner: HACKER, flags: "r") = "mi2";
  property sennight (owner: HACKER, flags: "r") = "1 week";
  property shippington (owner: HACKER, flags: "r") = "40 ft3";
  property shorthundredweight (owner: HACKER, flags: "r") = "100 lb";
  property shortquarter (owner: HACKER, flags: "r") = "500 lb";
  property shortton (owner: HACKER, flags: "r") = "2000 lb";
  property siemens (owner: HACKER, flags: "r") = "mho";
  property sigma (owner: HACKER, flags: "r") = "microsec";
  property sign (owner: HACKER, flags: "r") = "1|12 circle";
  property skein (owner: HACKER, flags: "r") = "360 feet";
  property skot (owner: HACKER, flags: "r") = "1e-3 apostilb";
  property slug (owner: HACKER, flags: "r") = "lb g sec2/ft";
  property span (owner: HACKER, flags: "r") = "9 in";
  property spat (owner: HACKER, flags: "r") = "sphere";
  property sphere (owner: HACKER, flags: "r") = "4 pi steradian";
  property spindle (owner: HACKER, flags: "r") = "14400 yd";
  property square (owner: HACKER, flags: "r") = "100 ft2";
  property sr (owner: HACKER, flags: "r") = "steradian";
  property statcoul (owner: HACKER, flags: "r") = "3.335635e-10 coul";
  property statfarad (owner: HACKER, flags: "r") = "1.112646e-12 farad";
  property stathenry (owner: HACKER, flags: "r") = "8.987584e11 henry";
  property statvolt (owner: HACKER, flags: "r") = "299.7930 volt";
  property steradian (owner: HACKER, flags: "r") = "radian radian";
  property stere (owner: HACKER, flags: "r") = "m3";
  property sthene (owner: HACKER, flags: "r") = "1e3 nt";
  property stilb (owner: HACKER, flags: "r") = "cd/cm2";
  property stoke (owner: HACKER, flags: "r") = "1 cm2/sec";
  property stone (owner: HACKER, flags: "r") = "14 lb";
  property tablespoon (owner: HACKER, flags: "r") = "4 fldr";
  property tbsp (owner: #2, flags: "r") = "tablespoon";
  property teaspoon (owner: HACKER, flags: "r") = "1|3 tablespoon";
  property tesla (owner: HACKER, flags: "r") = "weber/m2";
  property thermie (owner: HACKER, flags: "r") = "1e6 cal";
  property timberfoot (owner: HACKER, flags: "r") = "ft3";
  property tnt (owner: HACKER, flags: "r") = "4.6e6 m2/sec2";
  property ton (owner: HACKER, flags: "r") = "shortton";
  property tonne (owner: HACKER, flags: "r") = "1e6 gram";
  property torr (owner: HACKER, flags: "r") = "mm hg";
  property township (owner: HACKER, flags: "r") = "36 mi2";
  property tsp (owner: #2, flags: "r") = "teaspoon";
  property tun (owner: HACKER, flags: "r") = "252 gal";
  property turn (owner: HACKER, flags: "r") = "2 pi radian";
  property us (owner: HACKER, flags: "r") = "microsec";
  property usdram (owner: HACKER, flags: "r") = "1|8 oz";
  property v (owner: HACKER, flags: "r") = "volt";
  property volt (owner: HACKER, flags: "r") = "watt/amp";
  property water (owner: HACKER, flags: "r") = "0.22491|2.54 kg/m2 sec2";
  property watt (owner: HACKER, flags: "r") = "joule/sec";
  property weber (owner: HACKER, flags: "r") = "volt sec";
  property week (owner: HACKER, flags: "r") = "7 day";
  property wey (owner: HACKER, flags: "r") = "252 lb";
  property word (owner: HACKER, flags: "r") = "4 byte";
  property xunit (owner: HACKER, flags: "r") = "1.00202e-13 m";
  property yard (owner: HACKER, flags: "r") = "3 ft";
  property yd (owner: HACKER, flags: "r") = "yard";
  property year (owner: HACKER, flags: "r") = "365.24219879 day";
  property yr (owner: HACKER, flags: "r") = "year";

  override aliases = {"Conversion Utils"};
  override description = "This is a utilities package for converting from one unit of measurement to another. Type 'help #770' for more details.";
  override help_msg = {
    "Utility verbs for converting from one unit of measure to another.",
    "",
    "Unusual conversions:",
    ":dd_to_dms => converts decimal (INT or FLOAT) Degrees into Degrees, Minutes,",
    "              and Seconds. (Also works for decimal Hours.)",
    ":dms_to_dd => converts from Degrees (or Hours), Minutes, and Seconds to",
    "              decimal Degrees (or Hours).",
    ":rect_to_polar => converts from cartesian (x,y) coordinates to polar.",
    ":polar_to_rect => converts from polar (r, theta) coordinates to cartesian.",
    ":F_to_C => converts from Fahrenheit to Celsius.",
    ":C_to_F => converts from Celsius to Fahrenheit.",
    ":C_to_K => converts from Celsius to Kelvin.",
    ":K_to_C => converts from Kelvin to Celsius.",
    ":F_to_R => converts from Fahrenheit to Rankine.",
    ":R_to_F => converts from Rankine to Fahrenheit.",
    "",
    "Standard conversions:",
    ":convert => takes two string inputs and attempts to determine the ",
    "            multiplicative conversion factor. See the verb help for details",
    "            and input format.\""
  };
  override import_export_id = "convert_utils";
  override object_size = {30721, 1084848672};

  verb "dd_to_dms dh_to_hms" (this none this) owner: HACKER flags: "rxd"
    ":dd_to_dms(INT|FLOAT <degrees>) => LIST {INT <degrees>, INT <minutes>, FLOAT <seconds>}";
    "This verb converts decimal degrees to degrees, minutes, and seconds.";
    dd = tofloat(args[1]);
    s = ((dd - tofloat(d = toint(dd))) * 60.0 - tofloat(m = toint((dd - tofloat(d)) * 60.0))) * 60.0;
    return {d, m, s};
  endverb

  verb "dms_to_dd hms_to_dh" (this none this) owner: HACKER flags: "rxd"
    ":dms_to_dd(INT|FLOAT <deg>, INT|FLOAT <min>, INT|FLOAT <sec>) => FLOAT <deg>";
    "This verb converts degrees/minutes/seconds to decimal degrees.";
    {d, m, s} = args[1..3];
    d = tofloat(d);
    m = tofloat(m);
    s = tofloat(s);
    return d + m / 60.0 + s / 3600.0;
  endverb

  verb rect_to_polar (this none this) owner: HACKER flags: "rxd"
    ":rect_to_polar(INT|FLOAT <x>, INT|FLOAT <y>) => FLOAT <radius>, FLOAT <angle>.";
    "This verb converts from rectangular (x,y) coordinates to polar (r, theta) coordinates.";
    {x, y} = args[1..2];
    x = tofloat(x);
    y = tofloat(y);
    return {sqrt(x * x + x * x), `atan(y, x) ! E_INVARG => 0.0'};
  endverb

  verb polar_to_rect (this none this) owner: HACKER flags: "rxd"
    ":polar_to_rect(INT|FLOAT <radius>, INT|FLOAT <angle>) => FLOAT <x>, FLOAT <y>";
    "This verb converts from polar (radius, angle) coordinates to rectangulat (x,y) coordinates.";
    {r, a} = args[1..2];
    r = tofloat(r);
    a = tofloat(a);
    return {(r = r / (1.0 + (z2 = (z = tan(a / 2.0)) * z))) * (1.0 - z2), r * 2.0 * z};
  endverb

  verb "F_to_C degF_to_degC" (this none this) owner: HACKER flags: "rxd"
    ":F_to_C(INT|FLOAT <Fahrenheit>) => FLOAT <Celsius>";
    "This verb converts Fahrenheit degrees to Celsius degrees.";
    return (tofloat(args[1]) - 32.0) / 1.8;
  endverb

  verb "C_to_F degC_to_degF" (this none this) owner: HACKER flags: "rxd"
    ":C_to_F(INT|FLOAT <Celsius>) => FLOAT <Fahrenheit>";
    "This verb converts Celsius degrees to Fahrenheit degrees.";
    return tofloat(args[1]) * 1.8 + 32.0;
  endverb

  verb convert (this none this) owner: HACKER flags: "rxd"
    ":convert(STR <units>, STR <units>) => FLOAT conversion factor | LIST errors.";
    "This verb attempts to compute the conversion factor between two sets of units. If the two inputs are of the same type (two speeds, two lengths, etc.), the value is returned. If the two inputs are not of the same type, a LIST is returned as follows: {1, {FLOAT <value>, STR <units>}. {FLOAT <value>, STR <units>}}. The 1 indicates that the two inputs were correctly formed. <value> is the conversion factor of the input into the basic <units>. This error output is useful for determining the basic structure and value of an unknown unit of measure. If either of the inputs can not be broken down to known units, a LIST is returned as follows: {0, STR <bad input>}.";
    "";
    "The format of the input strings is fairly straight forward: any multiplicative combination of units, ending in an optional digit to represent that unit is raised to a power, the whole of which is preceeded by an initial value. Examples: \"100 kg m/sec2\", \"35 joules\", \"2000 furlongs/fortnight\"";
    "";
    "Some example uses:";
    ";$convert_utils:convert(\"2000 furlongs/fortnight\", \"mph\")";
    "=> 0.744047619047619";
    ";$convert_utils:convert(\"kilowatt hours\", \"joules\")";
    "=> 3600000.0";
    "";
    ";$convert_utils:convert(\"furlongs\", \"mph\")";
    "=> {1, {201.168, \"m\"}, {044704, \"m / s\"}}";
    "";
    ";$convert_utils:convert(\"junk\", \"meters\")";
    "=> {0, \"junk\"}";
    {havestr, wantstr} = args;
    {havenum, havestr} = $string_utils:first_word(havestr);
    havestr = $string_utils:trimr(tostr(havenum, " ", strsub(havestr, " ", "")));
    wantstr = strsub(wantstr, " ", "");
    "Preceeding three lines added by GD (#110777) on 23-June-2007 to stop an annoying error when you try to convert to/from things like 'fluid ounces'.";
    have = this:_do_convert(havestr);
    want = this:_do_convert(wantstr);
    if (have && want && have[2] == want[2])
      return have[1] / want[1];
    elseif (have && want)
      return {1, {have[1], this:_format_units(@have[2])}, {want[1], this:_format_units(@want[2])}};
    else
      return {0, have ? wantstr | havestr};
    endif
  endverb

  verb _do_convert (this none this) owner: HACKER flags: "rxd"
    "THIS VERB IS NOT INTENDED FOR USER USAGE.";
    ":_do_convert is the workhorse of $convert_utils:convert and is based loosely upon the 'units' Perl script the ships with BSD Unix.";
    "Essentially, it breaks the input up into values and units, attempts to break each unit down into elementary (basic) units, modifies the value as it goes, until it has no more input or can not convert a unit into a basic unit.";
    instr = args[1];
    units = this.basic_units_template;
    value = 1.0;
    top = 1;
    "Ensure that the division mark is a spearate word.";
    instr = $string_utils:substitute(instr, {{"/", " / "}});
    while (instr)
      "Grab the next word to process";
      {first, instr} = $string_utils:first_word(instr);
      if (first == "/")
        "Now we're working with values under the division mark - units with negative exponents.";
        top = 1 - top;
        continue;
      elseif (match(first, "|"))
        "The word was a value expressed as a ratio. Compute the ratio and adjust the value accordingly.";
        value = this:_do_value(first, value, top);
        continue;
      elseif ($string_utils:is_integer(first) || $string_utils:is_float(first))
        "The word was a value. Adjust the accumulated value accordingly.";
        value = top ? value * tofloat(first) | value / tofloat(first);
        continue;
      elseif (match(first, "[0-9]$"))
        "The word ends with a digit, but isn't a value. It must be a powered unit. Expand it: cm3 => cm cm cm";
        subs = match(first, "%([a-zA-Z]+%)%([0-9]+%)");
        first = substitute("%1", subs);
        power = toint(substitute("%2", subs));
        while (power > 0)
          instr = first + " " + instr;
          power = power - 1;
        endwhile
        continue;
      else
        "Check to see if the word starts with one or more metric prefix and attempt to evaluate the prefix.";
        {first, value, top} = this:_try_metric_prefix(first, value, top);
        "Check to see if we have a basic unit. If so, adjust the apropriate unit count.";
        if (index = first in this.basic_units)
          units[index][2] = top ? units[index][2] + 1 | units[index][2] - 1;
          continue;
        elseif (prop = `this.(first) ! E_PROPNF => 0')
          "Check to see if this is a known unit. If so, convert it and adjust the value and units.";
          result = this:_do_convert(prop);
          value = top ? value * result[1] | value / result[1];
          for i in [1..length(units)]
            units[i][2] = top ? units[i][2] + result[2][i][2] | units[i][2] - result[2][i][2];
          endfor
          continue;
        elseif (first[$] == "s")
          "Check to see if this is a normal 's'-ending plural, and try to do the above checks again.";
          temp = first[1..$ - 1];
          if (index = temp in this.basic_units)
            units[index][2] = top ? units[index][2] + 1 | units[index][2] - 1;
            continue;
          elseif (prop = `this.(temp) ! E_PROPNF => 0')
            result = this:_do_convert(prop);
            value = top ? value * result[1] | value / result[1];
            for i in [1..length(units)]
              units[i][2] = top ? units[i][2] + result[2][i][2] | units[i][2] - result[2][i][2];
            endfor
            continue;
          endif
        endif
        "We were unable to find any conversion for the current word, so halt all operation and return 0.";
        return 0;
      endif
    endwhile
    "We were able to successfully convert each part of the input. Return the equivalent value and units.";
    return {value, units};
  endverb

  verb _try_metric_prefix (this none this) owner: HACKER flags: "rxd"
    "THIS VERB IS NOT INTENDED FOR USER USAGE.";
    ":_try_metric_prefix runs through the metrix multipliers and tries to match them against the beginning of the input string. If successful, the given value is adjusted appropritately, and the input string is modified. The verb loops until there are no more prefix matches. (Hence, \"kilodecameter\" can be matched with only one verb call.";
    "If anyone knows of other possibilities here, please let me know.";
    {first, value, top} = args;
    while (1)
      if (subs = match(first, "^yocto%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1e24 | value * 1e24;
        continue;
      endif
      if (subs = match(first, "^zepto%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1e21 | value * 1e21;
        continue;
      endif
      if (subs = match(first, "^atto%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1e18 | value * 1e18;
        continue;
      endif
      if (subs = match(first, "^femto%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1000000000000000.0 | value * 1000000000000000.0;
        continue;
      endif
      if (subs = match(first, "^pico%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1000000000000.0 | value * 1000000000000.0;
        continue;
      endif
      if (subs = match(first, "^nano%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1000000000.0 | value * 1000000000.0;
        continue;
      endif
      if (match(first, "^micron"))
        break;
      endif
      if (subs = match(first, "^micro%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1000000.0 | value * 1000000.0;
        continue;
      endif
      if (subs = match(first, "^milli%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 1000.0 | value * 1000.0;
        continue;
      endif
      if (subs = match(first, "^centi%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 100.0 | value * 100.0;
        continue;
      endif
      if (subs = match(first, "^deci%(.*%)"))
        first = substitute("%1", subs);
        value = top ? value / 10.0 | value * 10.0;
        continue;
      endif
      if (subs = match(first, "^%(deca%|deka%)%(.*%)"))
        first = substitute("%2", subs);
        value = !top ? value / 10.0 | value * 10.0;
        continue;
      endif
      if (subs = match(first, "^hecto%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 100.0 | value * 100.0;
        continue;
      endif
      if (subs = match(first, "^kilo%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1000.0 | value * 1000.0;
        continue;
      endif
      if (subs = match(first, "^mega%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1000000.0 | value * 1000000.0;
        continue;
      endif
      if (subs = match(first, "^giga%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1000000000.0 | value * 1000000000.0;
        continue;
      endif
      if (subs = match(first, "^tera%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1000000000000.0 | value * 1000000000000.0;
        continue;
      endif
      if (subs = match(first, "^peta%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1000000000000000.0 | value * 1000000000000000.0;
        continue;
      endif
      if (subs = match(first, "^exa%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1e18 | value * 1e18;
        continue;
      endif
      if (subs = match(first, "^zetta%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1e21 | value * 1e21;
        continue;
      endif
      if (subs = match(first, "^yotta%(.*%)"))
        first = substitute("%1", subs);
        value = !top ? value / 1e24 | value * 1e24;
        continue;
      endif
      break;
    endwhile
    return {first, value, top};
  endverb

  verb _format_units (this none this) owner: HACKER flags: "rxd"
    "THIS VERB IS NOT INTENDED FOR USER USAGE.";
    ":_format_units takes the associative list of units and powers and construct a more user friendly string.";
    top = bottom = "";
    for pair in (args)
      if (pair[2] > 0)
        top = tostr(top, " ", pair[1], pair[2] > 1 ? pair[2] | "");
      elseif (pair[2] < 0)
        bottom = tostr(bottom, " ", pair[1], pair[2] < -1 ? -pair[2] | "");
      endif
    endfor
    if (bottom)
      return (top + " /" + bottom)[2..$];
    else
      return top[2..$];
    endif
  endverb

  verb "K_to_C degK_to_degC" (this none this) owner: HACKER flags: "rxd"
    ":K_to_C (INT|FLOAT <Kelvin>) => FLOAT <Celcius>";
    "This verb converts Kelvin degrees to Celcius degrees.";
    return tofloat(args[1]) - 273.0;
  endverb

  verb "C_to_K degC_to_degK" (this none this) owner: HACKER flags: "rxd"
    ":C_to_K (INT|FLOAT <Celcius>) => FLOAT <Kelvin>";
    "This verb converts Celcius degrees to Kelvin degrees.";
    return tofloat(args[1]) + 273.0;
  endverb

  verb "F_to_R degF_to_degR" (this none this) owner: HACKER flags: "rxd"
    ":F_to_R (INT|FLOAT <Fahrenheit>) => FLOAT <Rankine>";
    "This verb converts Fahrenheit degrees to Rankine degrees.";
    return tofloat(args[1]) + 459.67;
  endverb

  verb "R_to_F degR_to_degF" (this none this) owner: HACKER flags: "rxd"
    ":R_to_F (INT|FLOAT <Rankine>) => FLOAT <Fahrenheit>";
    "This verb converts Rankine degrees to Fahrenheit degrees.";
    return tofloat(args[1]) - 459.67;
  endverb

  verb _do_value (this none this) owner: HACKER flags: "rxd"
    "THIS VERB IS NOT INTENDED FOR USER USAGE.";
    ":_do_value takes a string of the form <number>|<number>, interprets it as a ratio, and applies that ratio to the incoming 'value' accordingly with the 'top' input, and returns it back to the calling verb.";
    {first, value, top} = args;
    {numer, denom} = $string_utils:explode(first, "|");
    return top ? value * tofloat(numer) / tofloat(denom) | value * tofloat(denom) / tofloat(numer);
  endverb
endobject