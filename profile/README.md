# goxlr-profile-loader

This code attempts to parse a GoXLR profile.xml into a rust structure, with the ability to write that structure back to a profile.xml format. 

xml-rs was chosen, due to the somewhat interesting way that the GoXLR names attributes, as it allows for pulling attributes as Strings, allowing for more reusable code.
Serde was considered, but it can't write XML attributes, nor can it easily handle 'dynamically' named attributes without introducing custom walkers over the XML.

Currently, all known XML is parsing, and writing correctly, and the resulting files can be packed into a .goxlr file, and
successfully loaded into the Application on Windows.

A Java tool is currently available at https://github.com/FrostyCoolSlug/goxlr-profile-xml-validator which uses XmlUnit 
to read an original GoXLR profile, as well as a profile written by this tool and check all values and attributes are
consistent between the two (ignoring format changes noted below).

*Notes*:  
1) From experimentation, with all but one exception (track attribute ordering in `sample.rs`), the Windows application 
doesn't care about ordering of tags or attributes. As such, the format of a produced XML file will be different to that
which Windows will produce, but will still be functional.


2) The GoXLR Windows app can store floats with more precision than a `f64`, in those cases, the values are rounded to an
`f64` value, so minor accuracy is lost. The following attributes are affected:
   * `<scribbleX scribbleXalpha=`
   * `<sampleStackX track_YNormalizedGain=`


3) In some odd cases the Windows Application may write an integer value as a float. (this tends to occur on effects),
in these cases, the value will be loaded as the appropriate int and saved as such.


4) The Windows application is somewhat inconsistent with its colour handling, some colours are upper-case, some are lower
the code here will convert all colours to uppercase.
