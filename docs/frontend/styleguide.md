# Frontend Styleguide

## Muc tieu

CSS trong repo nay phai de doc, de doi, va khong duoc quay lai kieu "mot file lon chua moi thu".

Quy uoc hien tai:

- tach CSS theo `foundation`, `shared`, `blocks`
- uu tien class theo kieu BEM
- primitive dung chung thi de trong `shared`
- style theo feature thi de trong `blocks`

## Cau truc thu muc

```text
apps/desktop/src/styles/
  foundation/
    base.css
    tokens.css
  shared/
    primitives.css
  blocks/
    launcher.css
    recorder-panel.css
    permissions-panel.css
    shortcut-panel.css
    settings-panel.css
    sessions-panel.css
    hud.css
  index.css
```

## Phan loai file

`foundation/`

- chi chua token, reset, base element styles
- khong viet feature-specific styles o day

`shared/`

- chua primitive dung lai duoc
- vi du: `panel`, `button`, `chip`, `pill`, `kbd`, `eyebrow`
- primitive phai trung tinh, khong duoc gan logic cua mot feature cu the

`blocks/`

- moi block UI lon co file rieng
- block name phai map ro rang voi component hoac surface
- vi du: `launcher`, `recorder-panel`, `permissions-panel`, `hud`

## Quy uoc naming

Dung BEM theo huong nay:

- block: `.launcher`
- element: `.launcher__header`
- modifier: `.launcher--loading`
- element modifier: `.button--secondary`

Tranh:

- class qua chung chung nhu `.header`, `.card`, `.left`, `.red`
- class phu thuoc vi tri layout nhu `.top-box-2`

## Khi nao dung primitive

Dung primitive khi style do:

- lap lai o nhieu block
- khong mang y nghia business rieng
- co the dung lai ma khong can biet component nao dang render no

Vi du tot:

- `.panel`
- `.panel__header`
- `.button`
- `.button--secondary`
- `.chip`

Khong dua vao `shared/primitives.css` neu style do chi dung cho:

- recorder metrics
- permissions item actions
- launcher summary

Nhung style nhu vay phai de trong file block tuong ung.

## Khi nao tao block moi

Tao file block moi khi:

- co mot feature panel moi
- co mot window/surface moi
- co mot section lon co nhieu element noi bo

Khong tao block moi neu ban chi them:

- 1 modifier nho cho primitive
- 1 style cuc bo cho element da thuoc block hien tai

## Layout rule

Layout tong the cua launcher nam trong block `launcher`.

Quy tac:

- layout cap page de trong `launcher.css`
- layout ben trong mot panel de trong block cua panel do
- khong trut het grid/flex vao `shared/primitives.css`

## Responsive rule

Responsive de gan noi su dung nhat co the:

- responsive cho launcher de trong `launcher.css`
- responsive cho recorder panel de trong `recorder-panel.css`
- chi de primitive responsive trong `shared` neu no dung cho nhieu block

## Cac nguyen tac quan trong

- uu tien composition thay vi selector sau
- tranh selector nhu `.launcher .panel .title`
- khong style theo tag neu do la UI feature-specific
- khong viet override lung tung giua cac block
- moi class moi phai tra loi duoc: no la primitive hay block-specific

## Mau tham khao

Primitive:

```css
.button {
  min-height: 44px;
  padding: 0 18px;
  border-radius: 16px;
}

.button--secondary {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-main);
}
```

Block:

```css
.recorder-panel__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}
```

## Checklist khi them UI moi

1. Xac dinh day la primitive hay block.
2. Neu la block moi, tao file trong `styles/blocks/`.
3. Dat class theo BEM, khong dat ten theo vi tri.
4. Import file moi trong `styles/index.css`.
5. Chay `npm run lint` va `npm run build:web`.

## Khong lam

- khong them CSS moi vao mot file tong hop 500+ dong
- khong dat ten class mo ho
- khong viet mot block trong nhieu file neu khong co ly do ro rang
- khong dung modifier de che giau viec dang can mot block rieng
