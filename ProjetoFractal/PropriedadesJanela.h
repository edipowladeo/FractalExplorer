#ifndef PROPRIEDADESJANELA_H
#define PROPRIEDADESJANELA_H

#include "global_headers.h"
#include "Ccoordenadas.h"

constexpr bool ShaderDisponivel = true;

class CpropriedadesJanela
{
public:
	float TaxaZoom;
	float TaxaPan;
	CcoordenadasPlano PosicaoInicial;

	bool telaCheia;
	CCoordenadas2DI DimensoesTextura;
	CCoordenadas2DI Recobrimento;
	CCoordenadas2DI DimensoesMatrizCelulas;
	
	bool DebugCamadas;
	bool DebugCelulas;

	bool UsarShader;


	int MagInicial = 8;

	float NavegacaoVelocidade;
	float NavegacaoVelocidadeZoom;
	int   DuracaoFrameDesejada = DURACAOFRAMEDESEJADA;//ms 1/taxa de quadros

	sf::Vector2f DimensoesJanela;
	sf::Vector2f JanelaDesenho;
	sf::Vector2f JanelaAlocar;
	sf::Vector2f JanelaDesalocar;

	int MagAlocarMax;
	int MagAlocarMin;
	int MagDesalocarMax;
	int MagDesalocarMin;

	float RazaoTamJanelaAlocar;
	float RazaoTamJanelaDesenho;
	float RazaoTamJanelaDesalocar;

	float RazaoJanelasDebug;
	
	CpropriedadesJanela();
	void AtualizarLimites();


};

#endif
