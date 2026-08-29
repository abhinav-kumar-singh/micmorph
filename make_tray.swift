import Cocoa

func drawMicIcon(size: CGFloat, filename: String) {
    let image = NSImage(size: NSSize(width: size, height: size))
    image.lockFocus()
    
    NSColor.clear.set()
    NSRect(x: 0, y: 0, width: size, height: size).fill()
    
    let scale = size / 22.0
    let color = NSColor.black
    color.setFill()
    color.setStroke()
    
    // Capsule
    let capW: CGFloat = 7.0 * scale
    let capH: CGFloat = 11.0 * scale
    let capX: CGFloat = (size - capW) / 2.0
    let capY: CGFloat = size - capH - (2.5 * scale)
    let capPath = NSBezierPath(roundedRect: NSRect(x: capX, y: capY, width: capW, height: capH), xRadius: capW/2.0, yRadius: capW/2.0)
    capPath.fill()
    
    // Cradle
    let cradleW: CGFloat = 12.0 * scale
    let cradleH: CGFloat = 9.0 * scale
    let cradleX: CGFloat = (size - cradleW) / 2.0
    let cradleY: CGFloat = capY - (2.0 * scale)
    let cradlePath = NSBezierPath()
    cradlePath.lineWidth = 1.8 * scale
    cradlePath.lineCapStyle = .round
    let center = NSPoint(x: size / 2.0, y: cradleY + (cradleH / 2.0))
    let radius = cradleW / 2.0
    cradlePath.appendArc(withCenter: center, radius: radius, startAngle: 190, endAngle: 350, clockwise: true)
    cradlePath.stroke()
    
    // Stem
    let stemPath = NSBezierPath()
    stemPath.lineWidth = 1.8 * scale
    stemPath.lineCapStyle = .round
    stemPath.move(to: NSPoint(x: size / 2.0, y: center.y - radius))
    stemPath.line(to: NSPoint(x: size / 2.0, y: 2.5 * scale))
    stemPath.stroke()
    
    // Base
    let basePath = NSBezierPath()
    basePath.lineWidth = 1.8 * scale
    basePath.lineCapStyle = .round
    basePath.move(to: NSPoint(x: size / 2.0 - (4.5 * scale), y: 2.5 * scale))
    basePath.line(to: NSPoint(x: size / 2.0 + (4.5 * scale), y: 2.5 * scale))
    basePath.stroke()
    
    image.unlockFocus()
    
    if let tiff = image.tiffRepresentation,
       let rep = NSBitmapImageRep(data: tiff),
       let png = rep.representation(using: .png, properties: [:]) {
        try? png.write(to: URL(fileURLWithPath: filename))
    }
}

drawMicIcon(size: 22, filename: "src-tauri/icons/tray-icon.png")
drawMicIcon(size: 44, filename: "src-tauri/icons/tray-icon@2x.png")
