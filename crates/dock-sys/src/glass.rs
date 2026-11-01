//! Yuvarlak köşeli gerçek cam: Windows Composition ile pencerenin arkasına
//! masaüstünün bulanık görüntüsünü (HostBackdropBrush) koyar ve bunu yuvarlak
//! dikdörtgenle kırparız. Eski yöntem (SetWindowCompositionAttribute) Windows
//! 11'de pencere bölgesini yok sayıp dikdörtgen çizdiği için köşelerde siyah/beyaz
//! alanlar kalıyordu.
//!
//! Composition nesneleri iş parçacığına bağlıdır: buradaki tüm fonksiyonlar
//! Tauri ana iş parçacığında çağrılmalıdır.

use crate::hwnd;
use std::cell::RefCell;
use std::collections::HashMap;
use windows::core::{Interface, Result, BOOL};
use windows_numerics::{Vector2, Vector3};
use windows::System::{DispatcherQueue, DispatcherQueueController};
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionColorBrush, CompositionRoundedRectangleGeometry, Compositor, ContainerVisual,
    SpriteVisual,
};
use windows::UI::Color;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_HOSTBACKDROPBRUSH};
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DispatcherQueueOptions, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT,
};

struct Glass {
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
    backdrop: SpriteVisual,
    tint: CompositionColorBrush,
    shape: CompositionRoundedRectangleGeometry,
}

thread_local! {
    static COMPOSITOR: RefCell<Option<(Option<DispatcherQueueController>, Compositor)>> =
        const { RefCell::new(None) };
    static GLASS: RefCell<HashMap<isize, Glass>> = RefCell::new(HashMap::new());
}

fn compositor() -> Result<Compositor> {
    COMPOSITOR.with(|cell| {
        if let Some((_, c)) = cell.borrow().as_ref() {
            return Ok(c.clone());
        }
        // Composition bir DispatcherQueue ister; yoksa bu iş parçacığına kur.
        let controller = if DispatcherQueue::GetForCurrentThread().is_ok() {
            None
        } else {
            let opts = DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            };
            unsafe { CreateDispatcherQueueController(opts) }.ok()
        };
        let c = Compositor::new()?;
        *cell.borrow_mut() = Some((controller, c.clone()));
        Ok(c)
    })
}

/// Pencerenin arkasına cam katmanını kurar.
pub fn attach(window: isize) -> Result<()> {
    if GLASS.with(|g| g.borrow().contains_key(&window)) {
        return Ok(());
    }
    let comp = compositor()?;
    unsafe {
        let on = BOOL(1);
        let _ = DwmSetWindowAttribute(
            hwnd(window),
            DWMWA_USE_HOSTBACKDROPBRUSH,
            &on as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
    let interop: ICompositorDesktopInterop = comp.cast()?;
    // false: görseller pencere içeriğinin (WebView) arkasında kalsın.
    let target = unsafe { interop.CreateDesktopWindowTarget(hwnd(window), false)? };

    let full = Vector2 { X: 1.0, Y: 1.0 };
    let root = comp.CreateContainerVisual()?;
    root.SetRelativeSizeAdjustment(full)?;

    let backdrop = comp.CreateSpriteVisual()?;
    backdrop.SetRelativeSizeAdjustment(full)?;
    backdrop.SetBrush(&comp.CreateHostBackdropBrush()?)?;

    let tint = comp.CreateColorBrushWithColor(Color { A: 0, R: 0, G: 0, B: 0 })?;
    let tint_visual = comp.CreateSpriteVisual()?;
    tint_visual.SetRelativeSizeAdjustment(full)?;
    tint_visual.SetBrush(&tint)?;

    let children = root.Children()?;
    children.InsertAtTop(&backdrop)?;
    children.InsertAtTop(&tint_visual)?;

    let shape = comp.CreateRoundedRectangleGeometry()?;
    let clip = comp.CreateGeometricClipWithGeometry(&shape)?;
    root.SetClip(&clip)?;
    target.SetRoot(&root)?;

    GLASS.with(|g| {
        g.borrow_mut().insert(
            window,
            Glass {
                _target: target,
                _root: root,
                backdrop,
                tint,
                shape,
            },
        )
    });
    Ok(())
}

/// Boyut (fiziksel piksel), köşe yarıçapı, ek renk tabakası ve açık/kapalı.
pub fn update(window: isize, w: f32, h: f32, radius: f32, tint: (u8, u8, u8, u8), enabled: bool) -> Result<()> {
    GLASS.with(|g| {
        let g = g.borrow();
        let Some(glass) = g.get(&window) else {
            return Ok(());
        };
        glass.shape.SetSize(Vector2 { X: w, Y: h })?;
        glass.shape.SetCornerRadius(Vector2 { X: radius, Y: radius })?;
        glass.tint.SetColor(Color {
            R: tint.0,
            G: tint.1,
            B: tint.2,
            A: if enabled { tint.3 } else { 0 },
        })?;
        glass.backdrop.SetIsVisible(enabled)?;
        Ok(())
    })
}

/// Cam katmanının açılış/kapanış animasyonu: panel CSS'te büyüyerek belirip
/// küçülerek kaybolurken cam da aynı anda (alt-ortadan) ölçeklenip söner.
/// Eskiden cam tam boyutta kalıyor, panel kapandıktan sonra arkada asılı duruyordu.
pub fn animate(window: isize, show: bool, ms: i64) -> Result<()> {
    let comp = compositor()?;
    GLASS.with(|g| {
        let g = g.borrow();
        let Some(gl) = g.get(&window) else { return Ok(()) };
        let root = &gl._root;
        let size = gl.shape.Size()?;
        root.SetCenterPoint(Vector3 { X: size.X / 2.0, Y: size.Y, Z: 0.0 })?;
        let ease = if show {
            comp.CreateCubicBezierEasingFunction(Vector2 { X: 0.2, Y: 0.9 }, Vector2 { X: 0.25, Y: 1.0 })?
        } else {
            comp.CreateCubicBezierEasingFunction(Vector2 { X: 0.4, Y: 0.0 }, Vector2 { X: 0.9, Y: 0.6 })?
        };
        let (o0, o1, s0, s1) = if show { (0.0, 1.0, 0.96, 1.0) } else { (1.0, 0.0, 1.0, 0.95) };
        root.SetOpacity(o0)?;
        root.SetScale(Vector3 { X: s0, Y: s0, Z: 1.0 })?;
        let dur = windows::Foundation::TimeSpan { Duration: ms * 10_000 };
        let op = comp.CreateScalarKeyFrameAnimation()?;
        op.InsertKeyFrameWithEasingFunction(1.0, o1, &ease)?;
        op.SetDuration(dur)?;
        root.StartAnimation(&windows::core::HSTRING::from("Opacity"), &op)?;
        let sc = comp.CreateVector3KeyFrameAnimation()?;
        sc.InsertKeyFrameWithEasingFunction(1.0, Vector3 { X: s1, Y: s1, Z: 1.0 }, &ease)?;
        sc.SetDuration(dur)?;
        root.StartAnimation(&windows::core::HSTRING::from("Scale"), &sc)?;
        Ok(())
    })
}

pub fn is_attached(window: isize) -> bool {
    GLASS.with(|g| g.borrow().contains_key(&window))
}

pub fn detach(window: isize) {
    GLASS.with(|g| {
        g.borrow_mut().remove(&window);
    });
}
