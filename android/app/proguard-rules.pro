# Ponte JS -> Kotlin da tela de câmeras: os métodos anotados não podem sumir.
-keepclassmembers class br.com.ysafe.camfacil.Ponte {
    @android.webkit.JavascriptInterface <methods>;
}
