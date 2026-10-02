# Parity corpus

These fixtures and this table were recorded from the 2.0 kernel by a tool that 2.5
removed together with the 2.0 tree. They are frozen records that cannot be regenerated
from this tree (the tool and the 2.0 tree remain only in git history, last present at
commit c89b64b), so a change to them is a deliberate edit.

Accuracy: geometric=1e-8, intersection=1e-9, tessellation=0.01, exchange=1e-6. Frame: origin=[0,0,0], x=[1,0,0], y=[0,0,-1], z=[0,1,0].

| Fixture | Pinned id | Parameters |
|---|---|---|
| cuboid | cuboid | size=[2,1,3] |
| cylinder | cylinder | radius=1, height=2 |
| sphere | sphere | radius=1 |
| cone | cone | radius=1, height=2 |
| frustum | frustum | lowerRadius=1, upperRadius=0.4, height=2 |
| torus | torus | majorRadius=2, minorRadius=0.5 |
| annular-cylinder | annular-cylinder | innerRadius=0.4, outerRadius=1, height=2 |
| cylinder-with-hole | cylinder-with-hole | innerRadius=0.4, outerRadius=1, height=2 |
| circle | circle | radius=1, start=0, sweep=TAU |
| linear-extrusion | linear-extrusion | outer=[[-1,-0.5],[1,-0.5],[1,0.5],[-1,0.5]], height=2 |
| arc-edged-extrusion | arc-edged-extrusion | rounded outer profile, height=2 |
| arc-edged-extrusion-with-holes | arc-edged-extrusion-with-holes | rounded outer profile, square hole [-0.5,0.5], height=2 |
| box-union | box-union | box-host size=[3,3,3], box-cutter origin=[1.5,0.5,0.5] size=[3,3,3] |
| box-intersection | box-intersection | box-host size=[3,3,3], box-cutter origin=[1.5,0.5,0.5] size=[3,3,3] |
| box-cut | box-cut | box-host size=[3,3,3], box-cutter origin=[1.5,0.5,0.5] size=[3,3,3] |
| box-cavity | box-cavity | box-host size=[3,3,3], box-inner origin=[1,1,1] size=[1,1,1] |
| sphere-cut | sphere-cut | sphere-host radius=2, sphere-cutter origin=[0.7,0,0] radius=1 |
