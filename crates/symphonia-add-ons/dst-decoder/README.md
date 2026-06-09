# dst-decoder

This is a Rust implementation of the [MPEG-4 Part 3 Subpart 10](https://en.wikipedia.org/wiki/MPEG-4_Part_3) Direct Stream Transfer (DST) Audio compression format.

This was a lossless audio compression format used virtually nowhere except on Sony and Phillip's [Super Audio CD](https://en.wikipedia.org/wiki/Super_Audio_CD) format, to losslessly compress their 1-bit PDM DSD audio format such that they could fit both stereo and 5.1 channel audio streams on the same physical disk format.

This is based heavily on the (as far as I know, only) reference implementation in C, which has floated around the internet for years. An example of the C reference lib is [here](https://github.com/EuFlo/sacd-ripper/tree/master/libs/libdstdec).


The only information I have on this codec is the above, plus the following from one of the file headers, which is reproduced for the sake of crediting the original authors:

```
Lossless coding of 1-bit oversampled audio - DST (Direct Stream Transfer)

This software was originally developed by:

* Aad Rijnberg 
  Philips Digital Systems Laboratories Eindhoven 
  <aad.rijnberg@philips.com>

* Fons Bruekers
  Philips Research Laboratories Eindhoven
  <fons.bruekers@philips.com>
   
* Eric Knapen
  Philips Digital Systems Laboratories Eindhoven
  <h.w.m.knapen@philips.com> 

And edited by:

* Richard Theelen
  Philips Digital Systems Laboratories Eindhoven
  <r.h.m.theelen@philips.com>

in the course of development of the MPEG-4 Audio standard ISO-14496-1, 2 and 3.
This software module is an implementation of a part of one or more MPEG-4 Audio
tools as specified by the MPEG-4 Audio standard. ISO/IEC gives users of the
MPEG-4 Audio standards free licence to this software module or modifications
thereof for use in hardware or software products claiming conformance to the
MPEG-4 Audio standards. Those intending to use this software module in hardware
or software products are advised that this use may infringe existing patents.
The original developers of this software of this module and their company,
the subsequent editors and their companies, and ISO/EIC have no liability for
use of this software module or modifications thereof in an implementation.
Copyright is not released for non MPEG-4 Audio conforming products. The
original developer retains full right to use this code for his/her own purpose,
assign or donate the code to a third party and to inhibit third party from
using the code for non MPEG-4 Audio conforming products. This copyright notice
must be included in all copies of derivative works.

Copyright © 2004.

Source file: dst_ac.c (Arithmetic Coding part of the DST Coding)

Required libraries: <none>

Authors:
RT:  Richard Theelen, PDSL-labs Eindhoven <r.h.m.theelen@philips.com>

Changes:
08-Mar-2004 RT  Initial version
```
