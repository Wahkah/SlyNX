//! Transform lists (transforms.md 2), matrices and transform references (transforms.md 1).

use alloc::vec::Vec;

use formats::error::Result;
use formats::geometry::Affine;
use formats::reader::Reader;

/// Reads a 48-byte matrix: three axis images, then the translation (transforms.md 1).
pub fn read_matrix(r: &mut Reader<'_>) -> Result<Affine> {
    let cols = r.mat3()?;
    let t = r.vec3()?;
    Ok(Affine { cols, t })
}

/// What a transform reference names (transforms.md 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformRef {
    /// −1: the root (identity, or the placement of the copy the object belongs to).
    None,
    /// Transform `n` of the same object.
    Own(u16),
    /// Transform `n` of the enclosing object.
    Enclosing(u16),
    /// `0x2000`: the transform of the object whose embedded list is being read.
    Owner,
    /// Any other value (does not occur on the disc).
    Other(i16),
}

impl TransformRef {
    pub fn decode(v: i16) -> TransformRef {
        match v {
            -1 => TransformRef::None,
            0..=0x0FFF => TransformRef::Own(v as u16),
            0x1000..=0x1FFF => TransformRef::Enclosing((v & 0x0FFF) as u16),
            0x2000 => TransformRef::Owner,
            _ => TransformRef::Other(v),
        }
    }
}

/// One transform of a list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub parent: i16,
    pub kind: i16,
    pub id: i32,
    pub flags: u8,
    pub local: Affine,
}

/// A two-bone chain (animation.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chain {
    pub upper: i16,
    pub lower: i16,
    pub end: i16,
    pub effector: [f32; 3],
    pub handle: i16,
    pub target: [f32; 3],
}

/// A mirror pair (animation.md 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mirror {
    pub source: i16,
    pub target: i16,
    pub copy_position: bool,
    pub copy_rotation: bool,
}

/// The transform list at the start of a solid body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformList {
    pub transforms: Vec<Transform>,
    pub chains: Vec<Chain>,
    pub mirrors: Vec<Mirror>,
    pub followers: usize,
    pub references: [i16; 3],
}

impl TransformList {
    pub fn read(r: &mut Reader<'_>) -> Result<TransformList> {
        let n = r.i16()?;
        if n < 0 {
            return Err(r.corrupt("negative transform count"));
        }
        let mut transforms = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let parent = r.i16()?;
            let kind = r.i16()?;
            let id = r.i32()?;
            let flags = r.u8()?;
            if flags & 0x01 != 0 {
                r.skip(4)?;
            }
            if flags & 0x02 != 0 && flags & 0x01 == 0 {
                r.skip(8)?;
            }
            if flags & 0x04 != 0 {
                r.skip(1)?;
            }
            if flags & 0x08 != 0 {
                r.skip(1 + 4 + 40 + 1 + 1 + 4)?;
            }
            if flags & 0x10 != 0 {
                r.skip(1 + 4 + 4)?;
            }
            if flags & 0x20 != 0 {
                r.skip(12)?;
            }
            let local = read_matrix(r)?;
            transforms.push(Transform { parent, kind, id, flags, local });
        }
        let chains = (0..r.u8()?)
            .map(|_| -> Result<Chain> {
                Ok(Chain {
                    upper: r.i16()?,
                    lower: r.i16()?,
                    end: r.i16()?,
                    effector: r.vec3()?,
                    handle: r.i16()?,
                    target: r.vec3()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mirrors = r.i8()?;
        let mirrors = (0..mirrors.max(0))
            .map(|_| -> Result<Mirror> {
                Ok(Mirror {
                    source: r.i16()?,
                    target: r.i16()?,
                    copy_position: r.i8()? != 0,
                    copy_rotation: r.i8()? != 0,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let followers = r.i8()?.max(0) as usize;
        for _ in 0..followers {
            r.skip(1 + 16 + 24)?;
            let m = r.i8()?.max(0) as usize;
            r.skip(m * 2)?;
        }
        let references = [r.i16()?, r.i16()?, r.i16()?];
        Ok(TransformList { transforms, chains, mirrors, followers, references })
    }

    /// World matrices of every transform as loaded. `root` stands for reference −1 (and 0x2000), `enclosing` for
    /// the enclosing object's world matrices. A reference to a later transform or out of range resolves to `root`.
    pub fn world_matrices(&self, root: &Affine, enclosing: &[Affine]) -> Vec<Affine> {
        let mut out: Vec<Affine> = Vec::with_capacity(self.transforms.len());
        for t in &self.transforms {
            let parent = match TransformRef::decode(t.parent) {
                TransformRef::Own(n) => out.get(n as usize).copied().unwrap_or(*root),
                TransformRef::Enclosing(n) => enclosing.get(n as usize).copied().unwrap_or(*root),
                _ => *root,
            };
            out.push(parent.mul(&t.local));
        }
        out
    }
}

/// Resolves a reference against an object's world matrices (meshes, children's parents).
pub fn resolve(reference: i16, own: &[Affine], enclosing: &[Affine], root: &Affine) -> Affine {
    match TransformRef::decode(reference) {
        TransformRef::Own(n) => own.get(n as usize).copied().unwrap_or(*root),
        TransformRef::Enclosing(n) => enclosing.get(n as usize).copied().unwrap_or(*root),
        _ => *root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references() {
        assert_eq!(TransformRef::decode(-1), TransformRef::None);
        assert_eq!(TransformRef::decode(5), TransformRef::Own(5));
        assert_eq!(TransformRef::decode(0x1003), TransformRef::Enclosing(3));
        assert_eq!(TransformRef::decode(0x2000), TransformRef::Owner);
        assert_eq!(TransformRef::decode(0x3000), TransformRef::Other(0x3000));
    }

    fn put_matrix(d: &mut Vec<u8>, t: [f32; 3]) {
        for v in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, t[0], t[1], t[2]] {
            d.extend_from_slice(&v.to_le_bytes());
        }
    }

    #[test]
    fn list_and_composition() {
        let mut d = Vec::new();
        d.extend_from_slice(&3i16.to_le_bytes());
        // transform 0: no parent, kind 0, id -1, flags 0
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.extend_from_slice(&0i16.to_le_bytes());
        d.extend_from_slice(&(-1i32).to_le_bytes());
        d.push(0);
        put_matrix(&mut d, [1.0, 0.0, 0.0]);
        // transform 1: parent 0, flags 5 (one f32, one s8)
        d.extend_from_slice(&0i16.to_le_bytes());
        d.extend_from_slice(&1i16.to_le_bytes());
        d.extend_from_slice(&7i32.to_le_bytes());
        d.push(5);
        d.extend_from_slice(&[0; 5]);
        put_matrix(&mut d, [0.0, 0.0, 75.0]);
        // transform 2: parent: transform 1 of the enclosing object, flags 0x3A (bits 1, 3, 4, 5)
        d.extend_from_slice(&0x1001i16.to_le_bytes());
        d.extend_from_slice(&6i16.to_le_bytes());
        d.extend_from_slice(&7i32.to_le_bytes());
        d.push(0x3A);
        d.extend_from_slice(&[0; 8 + 51 + 9 + 12]);
        put_matrix(&mut d, [0.0, 2.0, 0.0]);
        d.push(0); // chains
        d.push(1); // one mirror
        d.extend_from_slice(&[10, 0, 73, 0, 0, 1]);
        d.push(1); // one follower chain with 2 references
        d.extend_from_slice(&[0; 41]);
        d.push(2);
        d.extend_from_slice(&[0; 4]);
        for v in [1i16, 2, 2] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        let mut r = Reader::new(&d);
        let l = TransformList::read(&mut r).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!(l.transforms.len(), 3);
        assert_eq!(l.mirrors, [Mirror { source: 10, target: 73, copy_position: false, copy_rotation: true }]);
        assert_eq!((l.followers, l.references), (1, [1, 2, 2]));
        let enclosing = [Affine::IDENTITY, Affine::new(Affine::IDENTITY.cols, [0.0, 0.0, 100.0])];
        let w = l.world_matrices(&Affine::IDENTITY, &enclosing);
        assert_eq!(w[1].t, [1.0, 0.0, 75.0]);
        assert_eq!(w[2].t, [0.0, 2.0, 100.0]);
    }
}
