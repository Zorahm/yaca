/**
 * Шрифты §2: Onest для интерфейса, JetBrains Mono для моноширинного.
 *
 * Своим файлом, а не списком импортов в каждой точке входа: окон два (§10), и
 * второе, забывшее половину начертаний, рисовалось бы системным шрифтом — той
 * же плотности, да не той.
 *
 * Начертания ровно те, что называет §2: 400/500/600/700 у текстового и
 * 400/500/600 у моноширинного. Заголовочного шрифта нет. Лишнее начертание —
 * это лишний файл в каждой загрузке.
 */

import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/onest/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
