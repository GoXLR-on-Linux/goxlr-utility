# goxlr-profile-loader

This code attempts to parse a GoXLR profile.xml into a rust structure, with the ability to write that structure back to a profile.xml format. 

xml-rs was chosen, due to the somewhat interesting way that the GoXLR names attributes, as it allows for pulling attributes as Strings, allowing for more reusable code.
Serde was considered, but it can't write XML attributes, nor can it easily handle 'dynamically' named attributes without introducing custom walkers over the XML.
