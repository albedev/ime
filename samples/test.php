<?php
declare(strict_types=1);

namespace App\Models;

/**
 * Un utente di prova: commento su più righe
 * per controllare i colori della sintassi.
 */
final class Utente
{
    private const RUOLO_BASE = 'lettore';

    public function __construct(
        private string $nome,
        private ?string $email = null,
        private array $ruoli = [self::RUOLO_BASE],
    ) {}

    public function saluta(): string
    {
        // interpolazione e stringhe
        $ora = (int) date('H');
        $saluto = $ora < 12 ? 'Buongiorno' : "Buonasera";
        return "{$saluto}, {$this->nome}!";
    }

    public function isAdmin(): bool
    {
        return in_array('admin', $this->ruoli, true);
    }

    public static function daArray(array $dati): self
    {
        return new self($dati['nome'] ?? 'anonimo', $dati['email'] ?? null);
    }
}

$utenti = array_map(fn(array $d) => Utente::daArray($d), [
    ['nome' => 'Alberto', 'email' => 'a@example.com'],
    ['nome' => 'Giulia'],
]);

function x() {
    echo "sbu";
}

$html = <<<HTML
<ul>
    <li>Utenti: {count($utenti)}</li>
</ul>
HTML;

foreach ($utenti as $i => $u) {
    printf("%d. %s\n", $i + 1, $u->saluta());
}
?>
<!-- HTML dopo il blocco PHP -->
<p class="nota"><?= htmlspecialchars($html) ?></p>
